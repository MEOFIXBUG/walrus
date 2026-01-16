# Hướng Dẫn Start Cluster và Test Sau Khi Thêm Code Mới

## Bước 1: Build và Start Cluster

Sau khi thêm code mới (API KEY và Retention Policy), bạn cần build lại và start cluster:

```bash
cd distributed-walrus

# Dừng cluster cũ nếu đang chạy
make cluster-down

# Build lại và start cluster với code mới
make cluster-bootstrap
```

Lệnh này sẽ:
1. Build lại Docker image với code mới
2. Start 3 nodes (node1, node2, node3)
3. Đợi các ports 9091-9093 sẵn sàng

## Bước 2: Kiểm Tra Cluster Đã Chạy

```bash
# Xem logs của cluster
make cluster-logs

# Hoặc kiểm tra từng node
docker logs walrus-1
docker logs walrus-2
docker logs walrus-3
```

Bạn sẽ thấy log như:
```
Retention policy enabled: 168 hours, 0 entries
Node 1 ready; waiting for ctrl-c
```

## Bước 3: Test API KEY Authentication

### Test với Python Script

```bash
# Chạy script test API KEY
python3 scripts/test_api_key.py
```

Script sẽ test:
- ✅ Authentication với API key đúng
- ❌ Từ chối khi không có API key
- ❌ Từ chối khi API key sai
- ✅ Các lệnh REGISTER, PUT, GET với authentication

### Test Thủ Công với CLI

**Cách 1: Sử dụng API key option (khuyến nghị)**

```bash
# Start CLI client với API key
cargo run --bin walrus-cli -- --addr 127.0.0.1:9091 --api-key walrus-secret-key-123
```

CLI sẽ tự động authenticate và bạn có thể dùng các lệnh ngay:
```
> REGISTER test-topic
OK
> PUT test-topic "Hello World"
OK
> GET test-topic
OK Hello World
```

**Cách 2: Authenticate thủ công trong REPL**

```bash
# Start CLI client không có API key
cargo run --bin walrus-cli -- --addr 127.0.0.1:9091
```

Trong CLI, authenticate trước:
```
> AUTH walrus-secret-key-123
OK
> REGISTER test-topic
OK
> PUT test-topic "Hello World"
OK
> GET test-topic
OK Hello World
```

**Lưu ý:** Nếu không authenticate, bạn sẽ thấy lỗi:
```
ERR authentication required: send AUTH <api_key> first
```

## Bước 4: Test Retention Policy

Retention policy chạy background mỗi giờ. Để kiểm tra:

```bash
# Xem logs để thấy retention policy đang chạy
docker logs walrus-1 | grep -i retention

# Hoặc xem tất cả logs
make cluster-logs
```

Bạn sẽ thấy log như:
```
Retention policy: hours=168, entries=0
Retention cleanup completed
```

## Bước 5: Test với .NET Client

### Tạo Client với API KEY

```csharp
using Walrus.Client;

var client = new WalrusClient(
    host: "localhost",
    port: 9091,
    authToken: "walrus-secret-key-123"
);

// Client sẽ tự động authenticate khi connect
await client.RegisterTopicAsync("my-topic");
await client.PutAsync("my-topic", "Hello from .NET");
var data = await client.GetAsync("my-topic");
Console.WriteLine($"Received: {data}");
```

### Test với Configuration

```csharp
var options = new WalrusOptions
{
    Host = "localhost",
    Port = 9091,
    AuthToken = "walrus-secret-key-123"
};

var client = new WalrusEnhancedClient(
    options.Host,
    options.Port,
    TimeSpan.FromSeconds(options.ConnectionTimeoutSeconds),
    authToken: options.AuthToken
);
```

## Bước 6: Tùy Chỉnh API KEY và Retention

### Thay Đổi API KEY

Sửa file `docker-compose.yml`, tìm `--api-key` và thay đổi:

```yaml
command:
  - "--api-key"
  - "your-custom-api-key-here"  # Thay đổi ở đây
```

Sau đó restart:
```bash
make cluster-restart
```

### Thay Đổi Retention Policy

Sửa file `docker-compose.yml`:

```yaml
command:
  - "--retention-hours"
  - "24"           # Giữ dữ liệu 24 giờ
  - "--retention-entries"
  - "10000"        # Giữ tối đa 10000 entries
```

Sau đó restart:
```bash
make cluster-restart
```

## Bước 7: Chạy Tests Hiện Có

Các test scripts hiện có vẫn hoạt động, nhưng cần cập nhật để hỗ trợ API KEY:

```bash
# Test logging
make cluster-test-logs

# Test rollover
make cluster-test-rollover

# Test resilience
make cluster-test-resilience
```

**Lưu ý:** Các test scripts cũ có thể cần cập nhật để gửi AUTH command trước.

## Troubleshooting

### Cluster không start được

```bash
# Kiểm tra ports có bị chiếm không
netstat -an | grep -E "9091|9092|9093|6001|6002|6003"

# Xóa data cũ và thử lại
make cluster-clean
make cluster-down
make cluster-bootstrap
```

### Authentication không hoạt động

1. Kiểm tra API key trong `docker-compose.yml` giống nhau ở tất cả nodes
2. Kiểm tra logs: `docker logs walrus-1 | grep -i auth`
3. Đảm bảo client gửi AUTH command trước các lệnh khác

### Retention policy không chạy

1. Kiểm tra logs: `docker logs walrus-1 | grep -i retention`
2. Đảm bảo `--retention-hours` hoặc `--retention-entries` > 0
3. Retention chạy mỗi giờ, đợi một chút để thấy log

## Tóm Tắt

1. **Start cluster:** `make cluster-bootstrap`
2. **Test API KEY:** `python3 scripts/test_api_key.py`
3. **Test với CLI:** `cargo run --bin walrus-cli -- --addr 127.0.0.1:9091`
4. **Test với .NET:** Sử dụng `WalrusClient` với `authToken`
5. **Xem logs:** `make cluster-logs`
6. **Tùy chỉnh:** Sửa `docker-compose.yml` và `make cluster-restart`

