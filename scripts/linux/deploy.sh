#!/bin/bash

# Waiver Exchange - Linux Production Deployment Script
# This script sets up the complete production environment on Ubuntu 22.04 LTS

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
APP_USER="waiver"
APP_DIR="/opt/waiver-exchange"
SERVICE_DIR="/etc/systemd/system"
NGINX_DIR="/etc/nginx/sites-available"
NGINX_ENABLED="/etc/nginx/sites-enabled"
LOG_DIR="/var/log/waiver-exchange"

echo -e "${BLUE}🚀 Waiver Exchange Production Deployment${NC}"
echo -e "${BLUE}=======================================${NC}"

# Check if running as root
if [[ $EUID -ne 0 ]]; then
   echo -e "${RED}❌ This script must be run as root (use sudo)${NC}"
   exit 1
fi

# Check Ubuntu version
if ! grep -q "22.04" /etc/os-release; then
    echo -e "${YELLOW}⚠️  Warning: This script is designed for Ubuntu 22.04 LTS${NC}"
    read -p "Continue anyway? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

echo -e "${GREEN}✅ System check passed${NC}"

# Update system packages
echo -e "${BLUE}📦 Updating system packages...${NC}"
apt update && apt upgrade -y

# Install system dependencies
echo -e "${BLUE}📦 Installing system dependencies...${NC}"
apt install -y \
    curl \
    wget \
    git \
    build-essential \
    pkg-config \
    libssl-dev \
    postgresql \
    postgresql-contrib \
    redis-server \
    nginx \
    ufw \
    certbot \
    python3-certbot-nginx \
    htop \
    unzip

# Install Rust
echo -e "${BLUE}🦀 Installing Rust...${NC}"
if ! command -v rustc &> /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source ~/.cargo/env
    rustup default stable
    rustup target add x86_64-unknown-linux-gnu
else
    echo -e "${GREEN}✅ Rust already installed${NC}"
fi

# Create application user
echo -e "${BLUE}👤 Creating application user...${NC}"
if ! id "$APP_USER" &>/dev/null; then
    useradd -r -s /bin/false -d "$APP_DIR" "$APP_USER"
    echo -e "${GREEN}✅ Created user: $APP_USER${NC}"
else
    echo -e "${GREEN}✅ User $APP_USER already exists${NC}"
fi

# Create application directory
echo -e "${BLUE}📁 Creating application directory...${NC}"
mkdir -p "$APP_DIR"
mkdir -p "$LOG_DIR"
chown -R "$APP_USER:$APP_USER" "$APP_DIR"
chown -R "$APP_USER:$APP_USER" "$LOG_DIR"

# Configure PostgreSQL
echo -e "${BLUE}🐘 Configuring PostgreSQL...${NC}"
systemctl start postgresql
systemctl enable postgresql

# Create database and user
sudo -u postgres psql << EOF
CREATE DATABASE waiver_exchange;
CREATE USER waiver WITH ENCRYPTED PASSWORD 'waiver_secure_password_2024';
GRANT ALL PRIVILEGES ON DATABASE waiver_exchange TO waiver;
ALTER USER waiver CREATEDB;
\q
EOF

# Configure Redis
echo -e "${BLUE}🔴 Configuring Redis...${NC}"
systemctl start redis-server
systemctl enable redis-server

# Configure firewall
echo -e "${BLUE}🔥 Configuring firewall...${NC}"
ufw --force enable
ufw allow ssh
ufw allow 80/tcp
ufw allow 443/tcp
ufw allow 8081/tcp  # WebSocket
ufw allow 8082/tcp  # OAuth
ufw allow 8083/tcp  # REST API
ufw allow 3000/tcp  # Frontend (if needed)

# Create environment file
echo -e "${BLUE}⚙️  Creating environment configuration...${NC}"
cat > "$APP_DIR/.env" << EOF
# Database Configuration
DATABASE_URL=postgresql://waiver:waiver_secure_password_2024@localhost:5432/waiver_exchange
REDIS_URL=redis://localhost:6379

# OAuth Configuration (Set these environment variables before running)
# GOOGLE_CLIENT_ID=your_google_client_id_here
# GOOGLE_CLIENT_SECRET=your_google_client_secret_here
GOOGLE_REDIRECT_URL=https://waiver.exchange/auth/callback
# JWT_SECRET=your_super_secret_jwt_key_that_is_long_enough_for_hs256_algorithm_production_2024

# Application Configuration
WAIVER_DEV_MODE=false
WAIVER_LOG_LEVEL=info
WAIVER_LOG_FORMAT=json
WAIVER_MAX_SYMBOLS=1000
WAIVER_DATA_DIR=$APP_DIR/data

# Cache Configuration
CACHE_TTL_SECONDS=300

# Fantasy Points Configuration
FANTASY_POINTS_CONVERSION_RATE=1000

# Sleeper API Configuration
SLEEPER_API_BASE_URL=https://api.sleeper.app/v1
SLEEPER_API_KEY=your-sleeper-api-key-here

# Reservation Configuration
RESERVATION_EXPIRY_DAYS=7
EOF

chown "$APP_USER:$APP_USER" "$APP_DIR/.env"
chmod 600 "$APP_DIR/.env"

echo -e "${GREEN}✅ Environment configuration created${NC}"

# Create systemd service files
echo -e "${BLUE}⚙️  Creating systemd service files...${NC}"

# Waiver Exchange Service (WebSocket + OrderGateway)
cat > "$SERVICE_DIR/waiver-exchange.service" << EOF
[Unit]
Description=Waiver Exchange Trading Engine
After=network.target postgresql.service redis.service
Requires=postgresql.service redis.service

[Service]
Type=simple
User=$APP_USER
Group=$APP_USER
WorkingDirectory=$APP_DIR
Environment=RUST_LOG=info
EnvironmentFile=$APP_DIR/.env
ExecStart=$APP_DIR/waiver-exchange
Restart=always
RestartSec=5
StandardOutput=append:$LOG_DIR/waiver-exchange.log
StandardError=append:$LOG_DIR/waiver-exchange.log

[Install]
WantedBy=multi-user.target
EOF

# REST API Service
cat > "$SERVICE_DIR/waiver-rest-api.service" << EOF
[Unit]
Description=Waiver Exchange REST API
After=network.target postgresql.service redis.service waiver-exchange.service
Requires=postgresql.service redis.service

[Service]
Type=simple
User=$APP_USER
Group=$APP_USER
WorkingDirectory=$APP_DIR
Environment=RUST_LOG=info
EnvironmentFile=$APP_DIR/.env
ExecStart=$APP_DIR/rest-server
Restart=always
RestartSec=5
StandardOutput=append:$LOG_DIR/rest-api.log
StandardError=append:$LOG_DIR/rest-api.log

[Install]
WantedBy=multi-user.target
EOF

# OAuth Service
cat > "$SERVICE_DIR/waiver-oauth.service" << EOF
[Unit]
Description=Waiver Exchange OAuth Server
After=network.target postgresql.service redis.service
Requires=postgresql.service redis.service

[Service]
Type=simple
User=$APP_USER
Group=$APP_USER
WorkingDirectory=$APP_DIR
Environment=RUST_LOG=info
EnvironmentFile=$APP_DIR/.env
ExecStart=$APP_DIR/oauth-server
Restart=always
RestartSec=5
StandardOutput=append:$LOG_DIR/oauth.log
StandardError=append:$LOG_DIR/oauth.log

[Install]
WantedBy=multi-user.target
EOF

# Frontend Service (if needed)
cat > "$SERVICE_DIR/waiver-frontend.service" << EOF
[Unit]
Description=Waiver Exchange Frontend
After=network.target

[Service]
Type=simple
User=$APP_USER
Group=$APP_USER
WorkingDirectory=$APP_DIR/frontend
Environment=NODE_ENV=production
ExecStart=/usr/bin/npm start
Restart=always
RestartSec=5
StandardOutput=append:$LOG_DIR/frontend.log
StandardError=append:$LOG_DIR/frontend.log

[Install]
WantedBy=multi-user.target
EOF

# Reload systemd
systemctl daemon-reload

echo -e "${GREEN}✅ Systemd service files created${NC}"

# Create Nginx configuration
echo -e "${BLUE}🌐 Creating Nginx configuration...${NC}"
cat > "$NGINX_DIR/waiver-exchange" << EOF
# Waiver Exchange Nginx Configuration
server {
    listen 80;
    server_name waiver.exchange www.waiver.exchange;

    # Redirect HTTP to HTTPS
    return 301 https://\$server_name\$request_uri;
}

server {
    listen 443 ssl http2;
    server_name waiver.exchange www.waiver.exchange;

    # SSL Configuration (will be updated by certbot)
    ssl_certificate /etc/letsencrypt/live/waiver.exchange/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/waiver.exchange/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-RSA-AES256-GCM-SHA512:DHE-RSA-AES256-GCM-SHA512:ECDHE-RSA-AES256-GCM-SHA384:DHE-RSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers off;

    # Security headers
    add_header X-Frame-Options DENY;
    add_header X-Content-Type-Options nosniff;
    add_header X-XSS-Protection "1; mode=block";
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;

    # Frontend (Next.js)
    location / {
        proxy_pass http://localhost:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
        proxy_cache_bypass \$http_upgrade;
    }

    # REST API
    location /api/ {
        proxy_pass http://localhost:8083/;
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }

    # OAuth
    location /auth/ {
        proxy_pass http://localhost:8082/;
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }

    # WebSocket
    location /ws/ {
        proxy_pass http://localhost:8081/;
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }

    # Health check
    location /health {
        proxy_pass http://localhost:8083/health;
        access_log off;
    }
}
EOF

# Enable the site
ln -sf "$NGINX_DIR/waiver-exchange" "$NGINX_ENABLED/"
nginx -t && systemctl reload nginx

echo -e "${GREEN}✅ Nginx configuration created${NC}"

# Create management scripts
echo -e "${BLUE}📜 Creating management scripts...${NC}"

# Start all services script
cat > "$APP_DIR/start-all-services.sh" << 'EOF'
#!/bin/bash
echo "🚀 Starting Waiver Exchange services..."

# Start database and cache
sudo systemctl start postgresql redis-server

# Wait for dependencies
sleep 2

# Start application services
sudo systemctl start waiver-exchange
sudo systemctl start waiver-rest-api
sudo systemctl start waiver-oauth
sudo systemctl start waiver-frontend

echo "✅ All services started"
sudo systemctl status waiver-exchange waiver-rest-api waiver-oauth waiver-frontend --no-pager
EOF

# Stop all services script
cat > "$APP_DIR/stop-all-services.sh" << 'EOF'
#!/bin/bash
echo "🛑 Stopping Waiver Exchange services..."

sudo systemctl stop waiver-frontend
sudo systemctl stop waiver-oauth
sudo systemctl stop waiver-rest-api
sudo systemctl stop waiver-exchange

echo "✅ All services stopped"
EOF

# Health check script
cat > "$APP_DIR/health-check.sh" << 'EOF'
#!/bin/bash
echo "🏥 Waiver Exchange Health Check"
echo "================================"

# Check services
services=("waiver-exchange" "waiver-rest-api" "waiver-oauth" "waiver-frontend" "postgresql" "redis-server" "nginx")

for service in "${services[@]}"; do
    if systemctl is-active --quiet "$service"; then
        echo "✅ $service: Running"
    else
        echo "❌ $service: Not running"
    fi
done

echo ""
echo "🌐 Port Status:"
netstat -tlnp | grep -E ":(80|443|3000|5432|6379|8081|8082|8083)"

echo ""
echo "📊 System Resources:"
echo "CPU: $(top -bn1 | grep "Cpu(s)" | awk '{print $2}' | cut -d'%' -f1)%"
echo "Memory: $(free | grep Mem | awk '{printf("%.1f%%", $3/$2 * 100.0)}')"
echo "Disk: $(df -h / | awk 'NR==2{printf "%s", $5}')"
EOF

# View logs script
cat > "$APP_DIR/view-logs.sh" << 'EOF'
#!/bin/bash
echo "📋 Waiver Exchange Logs"
echo "======================="

if [ -z "$1" ]; then
    echo "Usage: $0 [service]"
    echo "Services: waiver-exchange, rest-api, oauth, frontend, all"
    exit 1
fi

case $1 in
    "waiver-exchange")
        sudo journalctl -u waiver-exchange -f
        ;;
    "rest-api")
        sudo journalctl -u waiver-rest-api -f
        ;;
    "oauth")
        sudo journalctl -u waiver-oauth -f
        ;;
    "frontend")
        sudo journalctl -u waiver-frontend -f
        ;;
    "all")
        sudo journalctl -u waiver-exchange -u waiver-rest-api -u waiver-oauth -u waiver-frontend -f
        ;;
    *)
        echo "Unknown service: $1"
        exit 1
        ;;
esac
EOF

# Make scripts executable
chmod +x "$APP_DIR"/*.sh
chown -R "$APP_USER:$APP_USER" "$APP_DIR"

echo -e "${GREEN}✅ Management scripts created${NC}"

# Create log rotation
echo -e "${BLUE}📋 Setting up log rotation...${NC}"
cat > "/etc/logrotate.d/waiver-exchange" << EOF
$LOG_DIR/*.log {
    daily
    missingok
    rotate 30
    compress
    delaycompress
    notifempty
    create 644 $APP_USER $APP_USER
    postrotate
        systemctl reload waiver-exchange waiver-rest-api waiver-oauth waiver-frontend > /dev/null 2>&1 || true
    endscript
}
EOF

echo -e "${GREEN}✅ Log rotation configured${NC}"

# Final instructions
echo -e "${GREEN}🎉 Deployment setup complete!${NC}"
echo ""
echo -e "${YELLOW}📋 Next Steps:${NC}"
echo "1. Copy your application binaries to $APP_DIR/"
echo "2. Set up your domain DNS to point to this server"
echo "3. Run: sudo certbot --nginx -d waiver.exchange -d www.waiver.exchange"
echo "4. Start services: $APP_DIR/start-all-services.sh"
echo "5. Check health: $APP_DIR/health-check.sh"
echo ""
echo -e "${YELLOW}📁 Important Directories:${NC}"
echo "- Application: $APP_DIR/"
echo "- Logs: $LOG_DIR/"
echo "- Config: $APP_DIR/.env"
echo "- Services: $SERVICE_DIR/"
echo ""
echo -e "${YELLOW}🔧 Management Commands:${NC}"
echo "- Start all: $APP_DIR/start-all-services.sh"
echo "- Stop all: $APP_DIR/stop-all-services.sh"
echo "- Health check: $APP_DIR/health-check.sh"
echo "- View logs: $APP_DIR/view-logs.sh [service]"
echo ""
echo -e "${GREEN}✅ Ready for application deployment!${NC}"
