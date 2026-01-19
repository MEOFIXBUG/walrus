#!/bin/bash
# Script để phân tích và xác định các file an toàn để xóa

echo "=== Phân tích Data Files trong Test Data ==="
echo ""

for node in node1 node2 node3; do
    data_dir="/home/user/walrus/distributed-walrus/test_data/$node/node_*/user_data/data_plane"

    echo "📁 $node:"

    if [ -d "$data_dir" ]; then
        for file in $data_dir/[0-9]*; do
            if [ -f "$file" ]; then
                filename=$(basename "$file")
                size=$(du -h "$file" | cut -f1)

                # Convert timestamp to readable date
                timestamp_ms="$filename"
                timestamp_s=$((timestamp_ms / 1000))
                date=$(date -d "@$timestamp_s" "+%Y-%m-%d %H:%M:%S" 2>/dev/null || echo "Invalid date")

                # Calculate age in hours
                now=$(date +%s)
                age_hours=$(( (now - timestamp_s) / 3600 ))

                echo "  📄 File: $filename"
                echo "     Size: $size"
                echo "     Created: $date"
                echo "     Age: ${age_hours} hours ago"

                # Check if file is likely safe to delete
                # (Old files that are much smaller than 1GB are probably sealed)
                size_mb=$(du -m "$file" | cut -f1)
                if [ "$age_hours" -gt 168 ] && [ "$size_mb" -lt 900 ]; then
                    echo "     ⚠️  Potential candidate for cleanup (>7 days old, <900MB)"
                fi

                echo ""
            fi
        done
    else
        echo "  ❌ Directory not found"
    fi
    echo ""
done

echo "=== Khuyến nghị ==="
echo "1. Files > 7 ngày và < 900MB: có thể là sealed segments, an toàn để xóa"
echo "2. Files gần 1GB hoặc rất mới: KHÔNG nên xóa"
echo "3. Tốt nhất: dùng retention policy tự động hoặc stop node trước khi xóa"
echo ""
echo "⚠️  CẢNH BÁO: Chỉ xóa file khi:"
echo "   - Node đã dừng hoàn toàn"
echo "   - Hoặc file đã được xác nhận là sealed và không active"
