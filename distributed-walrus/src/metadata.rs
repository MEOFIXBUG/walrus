use bytes::Bytes;
use octopii::StateMachineTrait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use crate::auth::{AuthManager, User};
use crate::retention::RetentionPolicy;

pub type NodeId = u64;
pub type TopicName = String;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClusterState {
    pub topics: HashMap<TopicName, TopicState>,
    /// Map node id -> advertised Raft/internal RPC address.
    #[serde(default)]
    pub nodes: HashMap<NodeId, String>,
    /// User authentication manager
    #[serde(default)]
    pub auth: AuthManager,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicState {
    pub current_segment: u64,
    pub leader_node: NodeId,
    /// Cumulative number of entries in all sealed segments
    #[serde(default)]
    pub last_sealed_entry_offset: u64,
    /// Map segment id -> number of entries in that sealed segment
    #[serde(default)]
    pub sealed_segments: HashMap<u64, u64>,
    /// Map segment id -> leader responsible for that segment
    #[serde(default)]
    pub segment_leaders: HashMap<u64, NodeId>,
    /// Retention policy for this topic (how long to keep data)
    #[serde(default)]
    pub retention: RetentionPolicy,
    /// Timestamp when each segment was created (for time-based retention)
    /// Map segment id -> creation timestamp (seconds since UNIX_EPOCH)
    #[serde(default)]
    pub segment_created_at: HashMap<u64, u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MetadataCmd {
    CreateTopic {
        name: String,
        initial_leader: NodeId,
    },
    RolloverTopic {
        name: String,
        new_leader: NodeId,
        sealed_segment_entry_count: u64,
    },
    UpsertNode {
        node_id: NodeId,
        addr: String,
    },
    AddUser {
        user: User,
    },
    RemoveUser {
        username: String,
    },
    SetRetention {
        topic: String,
        policy: RetentionPolicy,
    },
    DeleteSegments {
        topic: String,
        segment_ids: Vec<u64>,
    },
}

#[derive(Clone)]
pub struct Metadata {
    state: Arc<RwLock<ClusterState>>,
}

impl Metadata {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(ClusterState::default())),
        }
    }

    pub fn get_topic_state(&self, topic: &str) -> Option<TopicState> {
        let guard = self.state.read().ok()?;
        guard.topics.get(topic).cloned()
    }

    pub fn get_node_addr(&self, node_id: NodeId) -> Option<String> {
        let guard = self.state.read().ok()?;
        guard.nodes.get(&node_id).cloned()
    }

    pub fn all_node_addrs(&self) -> Vec<(NodeId, String)> {
        match self.state.read() {
            Ok(guard) => guard
                .nodes
                .iter()
                .map(|(id, addr)| (*id, addr.clone()))
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn owned_topics(&self, node_id: NodeId) -> Vec<(String, u64)> {
        let guard = match self.state.read() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        let mut out = Vec::new();
        for (topic, state) in guard.topics.iter() {
            if state.leader_node == node_id {
                out.push((topic.clone(), state.current_segment));
            }
        }
        out
    }

    pub fn sealed_count(&self, topic: &str, segment: u64) -> Option<u64> {
        let guard = self.state.read().ok()?;
        guard
            .topics
            .get(topic)
            .and_then(|t| t.sealed_segments.get(&segment).copied())
    }

    pub fn segment_leader(&self, topic: &str, segment: u64) -> Option<NodeId> {
        let guard = self.state.read().ok()?;
        guard
            .topics
            .get(topic)
            .and_then(|t| t.segment_leaders.get(&segment).copied())
            .or_else(|| guard.topics.get(topic).map(|t| t.leader_node))
    }

    pub fn authenticate(&self, username: &str, password: &str) -> Option<User> {
        let guard = self.state.read().ok()?;
        guard.auth.authenticate(username, password).ok().cloned()
    }

    /// Authenticate with API key (fast, requires write lock for index rebuild)
    pub fn authenticate_with_api_key(&self, api_key: &str) -> Option<User> {
        let mut guard = self.state.write().ok()?;
        guard.auth.authenticate_with_api_key(api_key).cloned()
    }

    pub fn user_exists(&self, username: &str) -> bool {
        self.state
            .read()
            .ok()
            .map(|g| g.auth.user_exists(username))
            .unwrap_or(false)
    }

    pub fn has_users(&self) -> bool {
        self.state
            .read()
            .ok()
            .map(|g| !g.auth.is_empty())
            .unwrap_or(false)
    }
}

impl StateMachineTrait for Metadata {
    fn apply(&self, command: &[u8]) -> Result<Bytes, String> {
        let cmd: MetadataCmd =
            bincode::deserialize(command).map_err(|e| format!("decode cmd: {e}"))?;
        let mut state = self
            .state
            .write()
            .map_err(|_| "state poisoned".to_string())?;

        match cmd {
            MetadataCmd::CreateTopic {
                name,
                initial_leader,
            } => {
                if state.topics.contains_key(&name) {
                    return Ok(Bytes::from_static(b"EXISTS"));
                }

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                let mut topic = TopicState {
                    current_segment: 1,
                    leader_node: initial_leader,
                    last_sealed_entry_offset: 0,
                    sealed_segments: HashMap::new(),
                    segment_leaders: HashMap::new(),
                    retention: RetentionPolicy::default(),
                    segment_created_at: HashMap::new(),
                };
                topic.segment_leaders.insert(1, initial_leader);
                topic.segment_created_at.insert(1, now); // Record creation time
                state.topics.insert(name, topic);
                Ok(Bytes::from_static(b"CREATED"))
            }
            MetadataCmd::RolloverTopic {
                name,
                new_leader,
                sealed_segment_entry_count,
            } => {
                if let Some(topic_state) = state.topics.get_mut(&name) {
                    let sealed_seg = topic_state.current_segment;
                    topic_state
                        .sealed_segments
                        .insert(sealed_seg, sealed_segment_entry_count);
                    topic_state
                        .segment_leaders
                        .insert(sealed_seg, topic_state.leader_node);
                    topic_state.last_sealed_entry_offset += sealed_segment_entry_count;
                    topic_state.current_segment += 1;
                    topic_state.leader_node = new_leader;
                    topic_state
                        .segment_leaders
                        .insert(topic_state.current_segment, new_leader);

                    // Record creation timestamp for the new segment
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    topic_state.segment_created_at.insert(topic_state.current_segment, now);

                    return Ok(Bytes::from_static(b"ROLLED"));
                }
                Err("Topic not found".into())
            }
            MetadataCmd::UpsertNode { node_id, addr } => {
                state.nodes.insert(node_id, addr);
                Ok(Bytes::from_static(b"NODE"))
            }
            MetadataCmd::AddUser { user } => {
                state
                    .auth
                    .add_user(user)
                    .map(|_| Bytes::from_static(b"USER_ADDED"))
                    .map_err(|e| e.to_string())
            }
            MetadataCmd::RemoveUser { username } => {
                state
                    .auth
                    .remove_user(&username)
                    .map(|_| Bytes::from_static(b"USER_REMOVED"))
                    .map_err(|e| e.to_string())
            }
            MetadataCmd::SetRetention { topic, policy } => {
                if let Some(topic_state) = state.topics.get_mut(&topic) {
                    topic_state.retention = policy;
                    Ok(Bytes::from_static(b"RETENTION_SET"))
                } else {
                    Err("Topic not found".into())
                }
            }
            MetadataCmd::DeleteSegments { topic, segment_ids } => {
                if let Some(topic_state) = state.topics.get_mut(&topic) {
                    for seg_id in segment_ids {
                        topic_state.sealed_segments.remove(&seg_id);
                        topic_state.segment_leaders.remove(&seg_id);
                        topic_state.segment_created_at.remove(&seg_id);
                    }
                    Ok(Bytes::from_static(b"SEGMENTS_DELETED"))
                } else {
                    Err("Topic not found".into())
                }
            }
        }
    }

    fn snapshot(&self) -> Vec<u8> {
        let state = self.state.read().ok();
        bincode::serialize(state.as_deref().unwrap_or(&ClusterState::default())).unwrap_or_default()
    }

    fn restore(&self, data: &[u8]) -> Result<(), String> {
        let recovered: ClusterState =
            bincode::deserialize(data).map_err(|e| format!("snapshot decode: {e}"))?;
        let mut guard = self
            .state
            .write()
            .map_err(|_| "state poisoned".to_string())?;
        *guard = recovered;
        Ok(())
    }
}
