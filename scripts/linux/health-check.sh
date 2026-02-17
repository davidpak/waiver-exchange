#!/bin/bash

# Waiver Exchange - Health Check Script
# This script performs comprehensive health checks on all services

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🏥 Waiver Exchange Health Check${NC}"
echo -e "${BLUE}===============================${NC}"
echo ""

# Configuration
API_BASE_URL="http://localhost:8083"
OAUTH_BASE_URL="http://localhost:8082"
WS_BASE_URL="ws://localhost:8081"
FRONTEND_URL="http://localhost:3000"

# Function to check HTTP endpoint
check_http_endpoint() {
    local url=$1
    local service_name=$2
    local expected_status=${3:-200}
    
    echo -n "  Testing $service_name... "
    
    if response=$(curl -s -o /dev/null -w "%{http_code}" --connect-timeout 5 "$url" 2>/dev/null); then
        if [ "$response" = "$expected_status" ]; then
            echo -e "${GREEN}✅ OK (HTTP $response)${NC}"
            return 0
        else
            echo -e "${YELLOW}⚠️  Unexpected status: HTTP $response${NC}"
            return 1
        fi
    else
        echo -e "${RED}❌ Connection failed${NC}"
        return 1
    fi
}

# Function to check service status
check_service_status() {
    local service_name=$1
    local description=$2
    
    echo -n "  $description... "
    
    if systemctl is-active --quiet "$service_name"; then
        echo -e "${GREEN}✅ Running${NC}"
        return 0
    else
        echo -e "${RED}❌ Not running${NC}"
        return 1
    fi
}

# Function to check port
check_port() {
    local port=$1
    local service_name=$2
    
    echo -n "  Port $port ($service_name)... "
    
    if netstat -tlnp | grep -q ":$port "; then
        echo -e "${GREEN}✅ Listening${NC}"
        return 0
    else
        echo -e "${RED}❌ Not listening${NC}"
        return 1
    fi
}

# Function to check database connection
check_database() {
    echo -n "  PostgreSQL connection... "
    
    if sudo -u postgres psql -d waiver_exchange -c "SELECT 1;" >/dev/null 2>&1; then
        echo -e "${GREEN}✅ Connected${NC}"
        return 0
    else
        echo -e "${RED}❌ Connection failed${NC}"
        return 1
    fi
}

# Function to check Redis connection
check_redis() {
    echo -n "  Redis connection... "
    
    if redis-cli ping >/dev/null 2>&1; then
        echo -e "${GREEN}✅ Connected${NC}"
        return 0
    else
        echo -e "${RED}❌ Connection failed${NC}"
        return 1
    fi
}

# Initialize counters
total_checks=0
passed_checks=0

# System Services Check
echo -e "${BLUE}🔧 System Services${NC}"
echo -e "${BLUE}==================${NC}"

services=("postgresql:PostgreSQL Database" "redis-server:Redis Cache" "nginx:Nginx Reverse Proxy")
frontend_configured=false

if systemctl list-unit-files | grep -q "waiver-frontend.service"; then
    services+=("waiver-frontend:Frontend Application")
    frontend_configured=true
fi

for service_info in "${services[@]}"; do
    service_name=$(echo $service_info | cut -d: -f1)
    description=$(echo $service_info | cut -d: -f2)
    
    ((total_checks++))
    if check_service_status "$service_name" "$description"; then
        ((passed_checks++))
    fi
done

echo ""

# Application Services Check
echo -e "${BLUE}🚀 Application Services${NC}"
echo -e "${BLUE}=======================${NC}"

app_services=("waiver-exchange:Waiver Exchange Trading Engine" "waiver-rest-api:REST API Server" "waiver-oauth:OAuth Authentication Server")

for service_info in "${app_services[@]}"; do
    service_name=$(echo $service_info | cut -d: -f1)
    description=$(echo $service_info | cut -d: -f2)
    
    ((total_checks++))
    if check_service_status "$service_name" "$description"; then
        ((passed_checks++))
    fi
done

echo ""

# Port Status Check
echo -e "${BLUE}🌐 Port Status${NC}"
echo -e "${BLUE}==============${NC}"

ports=("80:HTTP" "443:HTTPS" "5432:PostgreSQL" "6379:Redis" "8081:WebSocket" "8082:OAuth" "8083:REST API")
if [ "$frontend_configured" = true ]; then
    ports+=("3000:Frontend")
fi

for port_info in "${ports[@]}"; do
    port=$(echo $port_info | cut -d: -f1)
    service_name=$(echo $port_info | cut -d: -f2)
    
    ((total_checks++))
    if check_port "$port" "$service_name"; then
        ((passed_checks++))
    fi
done

echo ""

# Database Connectivity Check
echo -e "${BLUE}🗄️  Database Connectivity${NC}"
echo -e "${BLUE}=========================${NC}"

((total_checks++))
if check_database; then
    ((passed_checks++))
fi

((total_checks++))
if check_redis; then
    ((passed_checks++))
fi

echo ""

# API Endpoints Check
echo -e "${BLUE}🔗 API Endpoints${NC}"
echo -e "${BLUE}================${NC}"

# REST API Health Check
((total_checks++))
if check_http_endpoint "$API_BASE_URL/health" "REST API Health" 200; then
    ((passed_checks++))
fi

# OAuth Health Check
((total_checks++))
if check_http_endpoint "$OAUTH_BASE_URL/auth/google" "OAuth Google Endpoint" 302; then
    ((passed_checks++))
fi

# Frontend Health Check (if configured)
if [ "$frontend_configured" = true ]; then
    ((total_checks++))
    if check_http_endpoint "$FRONTEND_URL" "Frontend Application" 200; then
        ((passed_checks++))
    fi
fi

echo ""

# System Resources Check
echo -e "${BLUE}📊 System Resources${NC}"
echo -e "${BLUE}===================${NC}"

# CPU Usage
cpu_usage=$(top -bn1 | grep "Cpu(s)" | awk '{print $2}' | cut -d'%' -f1)
cpu_status="OK"
if (( $(echo "$cpu_usage > 80" | bc -l) )); then
    cpu_status="HIGH"
    echo -e "  CPU Usage: ${RED}${cpu_usage}% (HIGH)${NC}"
elif (( $(echo "$cpu_usage > 60" | bc -l) )); then
    cpu_status="MEDIUM"
    echo -e "  CPU Usage: ${YELLOW}${cpu_usage}% (MEDIUM)${NC}"
else
    echo -e "  CPU Usage: ${GREEN}${cpu_usage}% (OK)${NC}"
fi

# Memory Usage
memory_usage=$(free | grep Mem | awk '{printf("%.1f", $3/$2 * 100.0)}')
memory_status="OK"
if (( $(echo "$memory_usage > 80" | bc -l) )); then
    memory_status="HIGH"
    echo -e "  Memory Usage: ${RED}${memory_usage}% (HIGH)${NC}"
elif (( $(echo "$memory_usage > 60" | bc -l) )); then
    memory_status="MEDIUM"
    echo -e "  Memory Usage: ${YELLOW}${memory_usage}% (MEDIUM)${NC}"
else
    echo -e "  Memory Usage: ${GREEN}${memory_usage}% (OK)${NC}"
fi

# Disk Usage
disk_usage=$(df -h / | awk 'NR==2{print $5}' | sed 's/%//')
disk_status="OK"
if [ "$disk_usage" -gt 80 ]; then
    disk_status="HIGH"
    echo -e "  Disk Usage: ${RED}${disk_usage}% (HIGH)${NC}"
elif [ "$disk_usage" -gt 60 ]; then
    disk_status="MEDIUM"
    echo -e "  Disk Usage: ${YELLOW}${disk_usage}% (MEDIUM)${NC}"
else
    echo -e "  Disk Usage: ${GREEN}${disk_usage}% (OK)${NC}"
fi

echo ""

# Load Average
load_avg=$(uptime | awk -F'load average:' '{print $2}' | awk '{print $1}' | sed 's/,//')
echo -e "  Load Average: ${GREEN}${load_avg}${NC}"

echo ""

# Uptime
uptime_info=$(uptime -p)
echo -e "  System Uptime: ${GREEN}${uptime_info}${NC}"

echo ""

# Summary
echo -e "${BLUE}📋 Health Check Summary${NC}"
echo -e "${BLUE}=======================${NC}"

success_rate=$((passed_checks * 100 / total_checks))

if [ $success_rate -eq 100 ]; then
    echo -e "${GREEN}🎉 All checks passed! ($passed_checks/$total_checks)${NC}"
    echo -e "${GREEN}✅ Waiver Exchange is healthy and ready${NC}"
    exit_code=0
elif [ $success_rate -ge 80 ]; then
    echo -e "${YELLOW}⚠️  Most checks passed ($passed_checks/$total_checks)${NC}"
    echo -e "${YELLOW}🔧 Some issues detected, but system is operational${NC}"
    exit_code=1
else
    echo -e "${RED}❌ Multiple issues detected ($passed_checks/$total_checks)${NC}"
    echo -e "${RED}🚨 System requires attention${NC}"
    exit_code=2
fi

echo ""

# Resource Warnings
if [ "$cpu_status" = "HIGH" ] || [ "$memory_status" = "HIGH" ] || [ "$disk_status" = "HIGH" ]; then
    echo -e "${YELLOW}⚠️  Resource Warnings:${NC}"
    if [ "$cpu_status" = "HIGH" ]; then
        echo -e "  - High CPU usage detected"
    fi
    if [ "$memory_status" = "HIGH" ]; then
        echo -e "  - High memory usage detected"
    fi
    if [ "$disk_status" = "HIGH" ]; then
        echo -e "  - High disk usage detected"
    fi
    echo ""
fi

# Quick Actions
echo -e "${BLUE}🔧 Quick Actions${NC}"
echo -e "${BLUE}================${NC}"
echo "- View logs: ./view-logs.sh [service]"
echo "- Restart services: ./stop-all-services.sh && ./start-all-services.sh"
echo "- Check specific service: sudo systemctl status [service-name]"
echo "- View system logs: sudo journalctl -f"

echo ""

exit $exit_code
