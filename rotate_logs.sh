#!/bin/bash
# Arb Bot Log Rotation Script
# Rotates logs >50MB, compresses old logs, deletes compressed logs >30 days

set -e

BOT_DIR="/home/tom14cat14/Arb_Bot"
LOG_DIRS=("$BOT_DIR/logs" "$BOT_DIR/clean_arb_bot/logs")
MAX_SIZE_MB=50
DATE=$(date +%Y%m%d_%H%M%S)

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1"
}

log "=== Starting Arb Bot Log Rotation ==="

# Rotate large log files
for LOG_DIR in "${LOG_DIRS[@]}"; do
    if [ ! -d "$LOG_DIR" ]; then
        continue
    fi

    log "Checking directory: $LOG_DIR"

    for logfile in "$LOG_DIR"/*.log; do
        if [ ! -f "$logfile" ]; then
            continue
        fi

        BASENAME=$(basename "$logfile")
        SIZE_MB=$(du -m "$logfile" 2>/dev/null | cut -f1)

        if [ "$SIZE_MB" -gt "$MAX_SIZE_MB" ]; then
            ROTATED="${logfile%.log}_${DATE}.log"
            log "Rotating $BASENAME ($SIZE_MB MB) → $(basename "$ROTATED")"
            mv "$logfile" "$ROTATED"
            touch "$logfile"  # Create empty file for new logs
            gzip "$ROTATED"
            log "Compressed: $(basename "$ROTATED").gz"
        fi
    done
done

# Delete compressed logs older than 30 days
log "Cleaning up old compressed logs (>30 days)..."
DELETED=0
for LOG_DIR in "${LOG_DIRS[@]}"; do
    if [ ! -d "$LOG_DIR" ]; then
        continue
    fi

    COUNT=$(find "$LOG_DIR" -name "*.log.gz" -mtime +30 -delete -print | wc -l)
    DELETED=$((DELETED + COUNT))
done

log "Deleted $DELETED old compressed log files"
log "=== Log Rotation Complete ==="
