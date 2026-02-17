#!/bin/bash

# Waiver Exchange - Start All Services Script
# This script starts all Waiver Exchange services in the correct order

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 Starting Waiver Exchange Services${NC}"
echo -e "${BLUE}====================================${NC}"

# Check if running as root
if [[ $EUID -ne 0 ]]; then
   echo -e "${RED}❌ This script must be run as root (use sudo)${NC}"
   exit 1
fi

# Function to start a service and check status
start_service() {
    local service_name=$1
    local description=$2
    
    echo -e "${BLUE}📦 Starting $description...${NC}"
    
    if systemctl is-active --quiet "$service_name"; then
        echo -e "${GREEN}✅ $description is already running${NC}"
    else
        systemctl start "$service_name"
        sleep 2
        
        if systemctl is-active --quiet "$service_name"; then
            echo -e "${GREEN}✅ $description started successfully${NC}"
        else
            echo -e "${RED}❌ Failed to start $description${NC}"
            echo -e "${YELLOW}📋 Checking status:${NC}"
            systemctl status "$service_name" --no-pager
            exit 1
        fi
    fi
}

# Function to check if a service is ready
wait_for_service() {
    local service_name=$1
    local port=$2
    local max_attempts=30
    local attempt=1
    
    echo -e "${YELLOW}⏳ Waiting for $service_name to be ready...${NC}"
    
    while [ $attempt -le $max_attempts ]; do
        if netstat -tlnp | grep -q ":$port "; then
            echo -e "${GREEN}✅ $service_name is ready on port $port${NC}"
            return 0
        fi
        
        echo -e "${YELLOW}   Attempt $attempt/$max_attempts - waiting...${NC}"
        sleep 2
        ((attempt++))
    done
    
    echo -e "${RED}❌ $service_name failed to start on port $port after $max_attempts attempts${NC}"
    return 1
}

# Start database and cache services first
echo -e "${BLUE}🗄️  Starting database and cache services...${NC}"
start_service "postgresql" "PostgreSQL Database"
start_service "redis-server" "Redis Cache"

# Wait for database to be ready
wait_for_service "PostgreSQL" 5432
wait_for_service "Redis" 6379

# Start application services in dependency order
echo -e "${BLUE}🔧 Starting application services...${NC}"

# 1. Start main trading engine (WebSocket + OrderGateway)
start_service "waiver-exchange" "Waiver Exchange Trading Engine"
wait_for_service "Waiver Exchange" 8081

# 2. Start REST API server
start_service "waiver-rest-api" "REST API Server"
wait_for_service "REST API" 8083

# 3. Start OAuth server
start_service "waiver-oauth" "OAuth Authentication Server"
wait_for_service "OAuth Server" 8082

# 4. Start frontend (if enabled)
if systemctl list-unit-files | grep -q "waiver-frontend.service"; then
    start_service "waiver-frontend" "Frontend Application"
    wait_for_service "Frontend" 3000
else
    echo -e "${YELLOW}⚠️  Frontend service not configured, skipping...${NC}"
fi

# Start Nginx
start_service "nginx" "Nginx Reverse Proxy"

# Final status check
echo -e "${BLUE}📊 Final Service Status:${NC}"
echo -e "${BLUE}========================${NC}"

services=("postgresql" "redis-server" "waiver-exchange" "waiver-rest-api" "waiver-oauth" "nginx")
frontend_configured=false

if systemctl list-unit-files | grep -q "waiver-frontend.service"; then
    services+=("waiver-frontend")
    frontend_configured=true
fi

all_running=true

for service in "${services[@]}"; do
    if systemctl is-active --quiet "$service"; then
        echo -e "${GREEN}✅ $service: Running${NC}"
    else
        echo -e "${RED}❌ $service: Not running${NC}"
        all_running=false
    fi
done

echo ""
echo -e "${BLUE}🌐 Port Status:${NC}"
echo -e "${BLUE}===============${NC}"

ports=("80:HTTP" "443:HTTPS" "5432:PostgreSQL" "6379:Redis" "8081:WebSocket" "8082:OAuth" "8083:REST API")
if [ "$frontend_configured" = true ]; then
    ports+=("3000:Frontend")
fi

for port_info in "${ports[@]}"; do
    port=$(echo $port_info | cut -d: -f1)
    service_name=$(echo $port_info | cut -d: -f2)
    
    if netstat -tlnp | grep -q ":$port "; then
        echo -e "${GREEN}✅ Port $port ($service_name): Listening${NC}"
    else
        echo -e "${RED}❌ Port $port ($service_name): Not listening${NC}"
        all_running=false
    fi
done

echo ""
echo -e "${BLUE}📈 System Resources:${NC}"
echo -e "${BLUE}====================${NC}"

# CPU usage
cpu_usage=$(top -bn1 | grep "Cpu(s)" | awk '{print $2}' | cut -d'%' -f1)
echo -e "CPU Usage: ${GREEN}${cpu_usage}%${NC}"

# Memory usage
memory_usage=$(free | grep Mem | awk '{printf("%.1f%%", $3/$2 * 100.0)}')
echo -e "Memory Usage: ${GREEN}${memory_usage}${NC}"

# Disk usage
disk_usage=$(df -h / | awk 'NR==2{printf "%s", $5}')
echo -e "Disk Usage: ${GREEN}${disk_usage}${NC}"

echo ""

if [ "$all_running" = true ]; then
    echo -e "${GREEN}🎉 All services started successfully!${NC}"
    echo -e "${GREEN}✅ Waiver Exchange is ready for production${NC}"
    echo ""
    echo -e "${YELLOW}📋 Quick Commands:${NC}"
    echo "- Health check: ./health-check.sh"
    echo "- View logs: ./view-logs.sh [service]"
    echo "- Stop services: ./stop-all-services.sh"
    echo ""
    echo -e "${YELLOW}🌐 Access URLs:${NC}"
    echo "- Frontend: http://localhost:3000 (or your domain)"
    echo "- REST API: http://localhost:8083/api/"
    echo "- OAuth: http://localhost:8082/auth/"
    echo "- WebSocket: ws://localhost:8081/ws/"
else
    echo -e "${RED}❌ Some services failed to start${NC}"
    echo -e "${YELLOW}📋 Check logs for details:${NC}"
    echo "- sudo journalctl -u [service-name] -f"
    echo "- ./view-logs.sh [service]"
    exit 1
fi
