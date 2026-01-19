# Walrus Data Cleanup Guide

## Tổng quan

Walrus sử dụng các file dữ liệu có tên là timestamp (milliseconds) trong thư mục `data_plane/`. Mỗi file được pre-allocated **1GB** và chứa các segments của topics.

## Cấu trúc File

```
test_data/
├── node1/node_1/user_data/data_plane/
│   ├── 1768640164328        # Timestamp-based data file
│   ├── read_offset_idx_index.db
│   └── topic_clean_index.db
├── node2/node_2/user_data/data_plane/
└── node3/node_3/user_data/data_plane/
```

### Ý nghĩa các file:

- **Số timestamp (VD: 1768640164328)**: Data segments chứa messages
- **read_offset_idx_index.db**: Index cho read offsets
- **topic_clean_index.db**: Tracking clean state của topics

## Khi nào file AN TOÀN để xóa?

Một file CHỈ an toàn xóa khi đáp ứng **TẤT CẢ** điều kiện:

```rust
✓ Fully allocated      - File không nhận thêm blocks mới
✓ No locked blocks     - Không có write operation đang diễn ra
✓ All checkpointed     - Tất cả data đã fsync xuống disk
✓ Topic sealed         - Topic đã chuyển sang segment mới
✓ No active readers    - Không có consumer đang đọc
```

### Code reference:

```rust
// src/wal/runtime/allocator.rs:193
let ready_to_delete =
    fully_allocated &&
    locked == 0 &&
    total > 0 &&
    checkpointed >= total;
```

## Cơ chế Tự động Dọn dẹp

### 1. Background Deletion Thread

Walrus có background thread tự động xóa file an toàn:

**Workflow:**
1. File đủ điều kiện → gửi vào deletion queue
2. Background thread thu thập deletion requests
3. Sau 1000 fsync cycles, batch delete tất cả pending files
4. Đảm bảo không còn mmap/fd references trước khi xóa

**Code reference:**
- `src/wal/runtime/background.rs:167-194` - Deletion loop
- `src/wal/runtime/allocator.rs:188-200` - Eligibility check

### 2. Retention Policy (Đang phát triển)

**Configuration:**

```bash
# Khi start node
./distributed-walrus \
  --retention-hours 168      # Xóa segments > 7 ngày
  --retention-entries 0      # Unlimited entries (0 = disabled)
```

**Status hiện tại:**
- ✓ Configuration và background task đã implemented
- ⚠️ Logic xóa thực tế chưa hoàn thiện (placeholder)
- 📝 Cần thêm: list topics, track sealed segments, coordinate deletion

**Code reference:**
- `distributed-walrus/src/retention.rs` - Retention logic
- `distributed-walrus/src/config.rs:51-59` - Config params

## Hướng dẫn Dọn dẹp Thủ công

### ⚠️ QUAN TRỌNG: An toàn trước tiên

**TRƯỚC KHI xóa bất kỳ file nào:**

1. **Stop tất cả nodes:**
   ```bash
   pkill -f distributed-walrus
   # Verify: pgrep -f distributed-walrus (should return nothing)
   ```

2. **Backup dữ liệu quan trọng:**
   ```bash
   tar -czf backup-$(date +%Y%m%d).tar.gz test_data/
   ```

3. **Kiểm tra không có process nào đang access:**
   ```bash
   lsof | grep "test_data"
   ```

### Sử dụng Script Tự động

Chúng tôi cung cấp script an toàn:

```bash
# Dry-run (chỉ xem, không xóa)
cd distributed-walrus
DATA_ROOT=./test_data RETENTION_HOURS=168 ./scripts/safe_cleanup.sh

# Thực hiện xóa thật
DATA_ROOT=./test_data RETENTION_HOURS=168 DRY_RUN=false ./scripts/safe_cleanup.sh
```

### Quy tắc Xóa File

Script sử dụng các quy tắc:

| Điều kiện | Action | Lý do |
|-----------|--------|-------|
| File > retention_hours | 🗑️ DELETE | Quá cũ |
| File < 500MB và > 24h | 🗑️ DELETE | Likely sealed segment |
| File > 900MB | ✅ KEEP | Likely active, gần full allocation |
| File < 24h | ✅ KEEP | Quá mới, có thể đang active |

### Xóa Thủ công (Cẩn thận!)

Nếu bạn hiểu rõ và cần xóa manual:

```bash
# 1. Dừng nodes
pkill -f distributed-walrus

# 2. Xác định files cũ (>7 days)
cd test_data/node1/node_1/user_data/data_plane
find . -type f -name "[0-9]*" -mtime +7 -ls

# 3. Xóa files cũ
find . -type f -name "[0-9]*" -mtime +7 -delete

# 4. Restart nodes
cd ~/walrus/distributed-walrus
make run-cluster  # hoặc command bạn dùng
```

## Monitoring và Maintenance

### Kiểm tra Disk Usage

```bash
# Tổng size của data
du -sh test_data/

# Size per node
du -sh test_data/node*/

# Danh sách files lớn nhất
find test_data/ -type f -name "[0-9]*" -exec du -h {} \; | sort -rh | head -20
```

### Debug Logs

Enable debug logging để thấy deletion process:

```bash
RUST_LOG=debug ./distributed-walrus ...
# Look for: [reclaim] deletion requested
# Look for: [reclaim] deleted file
```

### Health Checks

```bash
# Kiểm tra index files integrity
ls -lh test_data/node*/node_*/user_data/data_plane/*.db

# Xác minh không có corrupted files
for f in test_data/node*/node_*/user_data/data_plane/[0-9]*; do
    file "$f"
done
```

## Best Practices

### 1. **Dùng Retention Policy (Recommended)**
```bash
# Production setup
--retention-hours 168    # 7 days
--retention-entries 0    # Time-based only
```

### 2. **Monitor trước khi đầy disk**
```bash
# Alert khi disk > 80%
df -h | awk '$5 > 80 {print "WARNING: " $0}'
```

### 3. **Định kỳ cleanup**
```bash
# Cron job (weekly cleanup)
0 2 * * 0 /path/to/safe_cleanup.sh
```

### 4. **Test trên non-production trước**
```bash
# Clone test_data
cp -r test_data test_data.backup

# Test cleanup
DRY_RUN=false ./scripts/safe_cleanup.sh

# Verify nodes still work
make test-cluster
```

## Troubleshooting

### "Cannot delete file: Device or resource busy"

**Nguyên nhân:** File đang được mmap hoặc có fd open

**Giải pháp:**
```bash
# 1. Tìm process đang dùng
lsof | grep <filename>

# 2. Stop process
kill -9 <PID>

# 3. Retry deletion
```

### "Deleted file but disk space not freed"

**Nguyên nhân:** Process còn file descriptor open

**Giải pháp:**
```bash
# Must restart all processes to release fd
pkill -f distributed-walrus
# Then start again
```

### "Node crashes after cleanup"

**Nguyên nhân:** Xóa nhầm active file

**Giải pháp:**
```bash
# Restore from backup
rm -rf test_data/
tar -xzf backup-YYYYMMDD.tar.gz

# Lesson: Always backup first!
```

## Implementation Status

| Feature | Status | Notes |
|---------|--------|-------|
| Background deletion thread | ✅ Complete | src/wal/runtime/background.rs |
| File state tracking | ✅ Complete | src/wal/runtime/allocator.rs |
| Retention config | ✅ Complete | distributed-walrus/src/config.rs |
| Retention background task | ✅ Complete | distributed-walrus/src/retention.rs |
| Retention enforcement | ⚠️ Placeholder | Needs: topic listing, segment tracking |
| Manual cleanup script | ✅ Complete | scripts/safe_cleanup.sh |

## Kế hoạch Phát triển

### Phase 1: Complete Retention Policy ✅
- [x] Add retention config params
- [x] Create retention module
- [x] Background cleanup task

### Phase 2: Retention Enforcement (TODO)
- [ ] List all topics from metadata
- [ ] Track sealed segments per topic
- [ ] Implement time-based deletion
- [ ] Implement entry-count-based deletion
- [ ] Add MetadataCmd for segment deletion

### Phase 3: Advanced Features (Future)
- [ ] Per-topic retention policies
- [ ] Compaction instead of deletion
- [ ] Remote storage archival
- [ ] Metrics và monitoring dashboard

## Resources

**Code References:**
- File lifecycle: `src/wal/runtime/allocator.rs`
- Background deletion: `src/wal/runtime/background.rs`
- Retention policy: `distributed-walrus/src/retention.rs`
- Configuration: `distributed-walrus/src/config.rs`

**Scripts:**
- Safe cleanup: `scripts/safe_cleanup.sh`
- Analysis: `scripts/analyze_data_files.sh`

**Documentation:**
- Setup guide: `README.md`
- API docs: `docs/API.md`

## Liên hệ

Nếu gặp vấn đề với data cleanup, hãy:
1. Check logs với `RUST_LOG=debug`
2. Review file states với `safe_cleanup.sh` dry-run
3. Backup trước khi thực hiện bất kỳ thay đổi nào
4. Test trên non-production environment trước

**Remember: An toàn > Nhanh. Backup > Xóa. Test > Production.**
