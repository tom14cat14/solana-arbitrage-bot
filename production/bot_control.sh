#!/bin/bash

# Arb Bot Control Script - Quick access to bot management

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

LOG_DIR="/home/tom14cat14/Arb_Bot/logs"
KILLSWITCH="/home/tom14cat14/Arb_Bot/KILLSWITCH"
WALLET="9WrFdecsvMogYEtwjGrCBs4LrfnZhm9QKigD4CdcD3kA"

# Show menu if no arguments
if [ $# -eq 0 ]; then
    echo "========================================="
    echo "Arb Bot Control Panel"
    echo "========================================="
    echo "Commands:"
    echo "  status    - Show bot status and recent activity"
    echo "  logs      - Tail bot logs"
    echo "  monitor   - Tail monitoring logs"
    echo "  health    - Show health check history"
    echo "  balance   - Check wallet balance and history"
    echo "  killswitch - Activate emergency killswitch"
    echo "  resume    - Deactivate killswitch"
    echo "  attach    - Attach to tmux session"
    echo "  restart   - Restart the monitoring session"
    echo "========================================="
    exit 0
fi

case "$1" in
    status)
        echo -e "${GREEN}=== Bot Status ===${NC}"
        if tmux has-session -t arb_bot 2>/dev/null; then
            echo -e "${GREEN}✅ Tmux session 'arb_bot' is running${NC}"
        else
            echo -e "${RED}❌ Tmux session 'arb_bot' not found${NC}"
        fi

        if [ -f "$KILLSWITCH" ]; then
            echo -e "${RED}🛑 KILLSWITCH ACTIVE${NC}"
        else
            echo -e "${GREEN}✅ Killswitch inactive${NC}"
        fi

        echo ""
        echo -e "${BLUE}=== Recent Activity (last 10 lines) ===${NC}"
        tail -10 "$LOG_DIR/autonomous_monitor.log" 2>/dev/null || echo "No logs found"
        ;;

    logs)
        echo -e "${BLUE}Tailing bot logs (Ctrl+C to exit)...${NC}"
        tail -f "$LOG_DIR/arb_bot.log"
        ;;

    monitor)
        echo -e "${BLUE}Tailing monitor logs (Ctrl+C to exit)...${NC}"
        tail -f "$LOG_DIR/autonomous_monitor.log"
        ;;

    health)
        echo -e "${GREEN}=== Health Check History (last 20) ===${NC}"
        tail -20 "$LOG_DIR/health_checks.log" 2>/dev/null | while IFS=',' read -r timestamp shredstream arb; do
            if [ "$shredstream" = "OK" ] && [ "$arb" = "OK" ]; then
                echo -e "$timestamp - ${GREEN}✅ All healthy${NC}"
            else
                echo -e "$timestamp - ${RED}❌ ShredStream: $shredstream, Arb: $arb${NC}"
            fi
        done || echo "No health logs found"
        ;;

    balance)
        echo -e "${GREEN}=== Current Balance ===${NC}"
        solana balance "$WALLET"
        echo ""
        echo -e "${BLUE}=== Balance History (last 10) ===${NC}"
        tail -10 "$LOG_DIR/balance_tracking.log" 2>/dev/null | while IFS=',' read -r timestamp balance; do
            echo "$timestamp - $balance SOL"
        done || echo "No balance history found"
        ;;

    killswitch)
        echo -e "${RED}🛑 ACTIVATING KILLSWITCH${NC}"
        touch "$KILLSWITCH"
        echo "$(date '+%Y-%m-%d %H:%M:%S') - Killswitch activated" >> "$LOG_DIR/killswitch.log"
        echo -e "${YELLOW}All bots will stop within 30 seconds${NC}"
        echo -e "To resume trading, run: ${GREEN}$0 resume${NC}"
        ;;

    resume)
        if [ -f "$KILLSWITCH" ]; then
            echo -e "${GREEN}Removing killswitch...${NC}"
            rm "$KILLSWITCH"
            echo "$(date '+%Y-%m-%d %H:%M:%S') - Killswitch deactivated" >> "$LOG_DIR/killswitch.log"
            echo -e "${GREEN}✅ Killswitch removed. Bots will resume automatically.${NC}"
        else
            echo -e "${YELLOW}Killswitch is not active${NC}"
        fi
        ;;

    attach)
        echo -e "${BLUE}Attaching to tmux session...${NC}"
        echo -e "${YELLOW}Press Ctrl+B then D to detach without stopping bots${NC}"
        sleep 2
        tmux attach -t arb_bot
        ;;

    restart)
        echo -e "${YELLOW}Restarting monitoring session...${NC}"
        tmux kill-session -t arb_bot 2>/dev/null || true
        sleep 2
        echo -e "${GREEN}Starting new session...${NC}"
        cd /home/tom14cat14/Arb_Bot/clean_arb_bot/production
        tmux new-session -d -s arb_bot './autonomous_monitor.sh'
        echo -e "${GREEN}✅ Bot restarted in tmux session 'arb_bot'${NC}"
        echo -e "Run '${BLUE}$0 attach${NC}' to view or '${BLUE}$0 logs${NC}' to tail logs"
        ;;

    *)
        echo -e "${RED}Unknown command: $1${NC}"
        echo "Run without arguments to see available commands"
        exit 1
        ;;
esac
