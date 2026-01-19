#!/bin/bash
# Script dọn dẹp AN TOÀN cho Walrus data files

set -e

echo "=== Walrus Safe Data Cleanup ==="
echo ""

# Configuration
DATA_ROOT="${DATA_ROOT:-./test_data}"
RETENTION_HOURS="${RETENTION_HOURS:-168}"  # 7 days default
DRY_RUN="${DRY_RUN:-true}"  # Safety: default to dry-run

# Convert hours to seconds for comparison
RETENTION_SECONDS=$((RETENTION_HOURS * 3600))
NOW_SECONDS=$(date +%s)

echo "📋 Configuration:"
echo "   Data root: $DATA_ROOT"
echo "   Retention: $RETENTION_HOURS hours ($(($RETENTION_HOURS / 24)) days)"
echo "   Dry run: $DRY_RUN"
echo ""

# Check if nodes are running
echo "🔍 Checking for running Walrus processes..."
if pgrep -f "distributed-walrus" > /dev/null; then
    echo "⚠️  WARNING: Walrus processes are running!"
    echo "   PIDs: $(pgrep -f "distributed-walrus" | tr '\n' ' ')"
    echo ""
    echo "❌ ABORTING: Stop all nodes before cleanup to avoid data corruption"
    echo "   Run: pkill -f distributed-walrus"
    exit 1
else
    echo "✓ No running processes found"
fi
echo ""

# Find and analyze all data files
echo "📁 Scanning for data files..."
TOTAL_SIZE=0
TOTAL_FILES=0
DELETE_SIZE=0
DELETE_COUNT=0

for node_dir in "$DATA_ROOT"/node*/node_*/user_data/data_plane; do
    if [ ! -d "$node_dir" ]; then
        continue
    fi

    echo ""
    echo "📂 Directory: $node_dir"

    for file in "$node_dir"/[0-9]*; do
        if [ ! -f "$file" ]; then
            continue
        fi

        filename=$(basename "$file")
        size_bytes=$(stat -c%s "$file" 2>/dev/null || stat -f%z "$file" 2>/dev/null)
        size_mb=$((size_bytes / 1024 / 1024))

        TOTAL_FILES=$((TOTAL_FILES + 1))
        TOTAL_SIZE=$((TOTAL_SIZE + size_mb))

        # Extract timestamp from filename (milliseconds)
        timestamp_ms="$filename"
        timestamp_s=$((timestamp_ms / 1000))

        # Calculate age
        age_seconds=$((NOW_SECONDS - timestamp_s))
        age_hours=$((age_seconds / 3600))
        age_days=$((age_hours / 24))

        # Format date (cross-platform compatible)
        file_date=$(date -d "@$timestamp_s" "+%Y-%m-%d %H:%M:%S" 2>/dev/null || date -r "$timestamp_s" "+%Y-%m-%d %H:%M:%S" 2>/dev/null || echo "N/A")

        echo "  📄 $filename"
        echo "     Size: ${size_mb}MB"
        echo "     Created: $file_date"
        echo "     Age: ${age_days}d ${age_hours}h"

        # Determine if should be deleted
        SHOULD_DELETE=false
        REASON=""

        # Rule 1: File older than retention period
        if [ "$age_seconds" -gt "$RETENTION_SECONDS" ]; then
            SHOULD_DELETE=true
            REASON="Exceeds retention period ($RETENTION_HOURS hours)"
        fi

        # Rule 2: Small old files (<500MB and >24h) likely sealed/abandoned
        if [ "$size_mb" -lt 500 ] && [ "$age_hours" -gt 24 ]; then
            SHOULD_DELETE=true
            REASON="${REASON:+$REASON; }Small old file (likely sealed segment)"
        fi

        # Rule 3: Files near 1GB are likely active - DO NOT DELETE
        if [ "$size_mb" -gt 900 ]; then
            SHOULD_DELETE=false
            REASON="Near 1GB allocation (likely active)"
        fi

        if [ "$SHOULD_DELETE" = true ]; then
            echo "     🗑️  WILL DELETE: $REASON"
            DELETE_COUNT=$((DELETE_COUNT + 1))
            DELETE_SIZE=$((DELETE_SIZE + size_mb))

            if [ "$DRY_RUN" != "true" ]; then
                rm -f "$file"
                echo "     ✓ Deleted"
            fi
        else
            echo "     ✓ Keep: $REASON"
        fi
    done
done

echo ""
echo "=== Summary ==="
echo "📊 Total files: $TOTAL_FILES (${TOTAL_SIZE}MB)"
echo "🗑️  To delete: $DELETE_COUNT files (${DELETE_SIZE}MB)"
echo "💾 To keep: $((TOTAL_FILES - DELETE_COUNT)) files ($((TOTAL_SIZE - DELETE_SIZE))MB)"
echo ""

if [ "$DRY_RUN" = "true" ]; then
    echo "ℹ️  DRY RUN MODE - No files were deleted"
    echo "   To actually delete, run: DRY_RUN=false $0"
else
    echo "✓ Cleanup completed"
fi

echo ""
echo "=== Safety Tips ==="
echo "1. Always stop nodes before manual cleanup"
echo "2. Use retention policy for automatic cleanup"
echo "3. Keep backups before aggressive cleanup"
echo "4. Monitor disk space: df -h $DATA_ROOT"
