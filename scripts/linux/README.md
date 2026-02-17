# Waiver Exchange - Linux Production Deployment

This directory contains all the scripts and configurations needed to deploy Waiver Exchange on a Linux VPS using systemd services.

## 🚀 Quick Start

### 1. Prerequisites
- Ubuntu 22.04 LTS VPS
- Root access or sudo privileges
- Domain name pointing to your server
- At least 2GB RAM, 2 CPU cores, 50GB SSD

### 2. Initial Setup
```bash
# Clone the repository
git clone <your-repo-url>
cd waiver-exchange

# Make scripts executable
chmod +x scripts/linux/*.sh

# Run the deployment script
sudo ./scripts/linux/deploy.sh
```

### 3. Deploy Application
```bash
# Copy your built binaries to the application directory
sudo cp target/release/waiver-exchange /opt/waiver-exchange/
sudo cp target/release/rest-server /opt/waiver-exchange/
sudo cp target/release/oauth-server /opt/waiver-exchange/

# Set proper ownership
sudo chown waiver:waiver /opt/waiver-exchange/*

# Start all services
sudo /opt/waiver-exchange/start-all-services.sh
```

### 4. SSL Certificate
```bash
# Get SSL certificate from Let's Encrypt
sudo certbot --nginx -d waiver.exchange -d www.waiver.exchange
```

## 📁 File Structure

```
scripts/linux/
├── deploy.sh                 # Main deployment script
├── start-all-services.sh     # Start all services
├── stop-all-services.sh      # Stop all services
├── health-check.sh           # Comprehensive health check
├── view-logs.sh              # View service logs
└── README.md                 # This file
```

## 🔧 Service Management

### Start Services
```bash
# Start all services
sudo /opt/waiver-exchange/start-all-services.sh

# Or start individual services
sudo systemctl start waiver-exchange
sudo systemctl start waiver-rest-api
sudo systemctl start waiver-oauth
sudo systemctl start waiver-frontend
```

### Stop Services
```bash
# Stop all services
sudo /opt/waiver-exchange/stop-all-services.sh

# Or stop individual services
sudo systemctl stop waiver-exchange
sudo systemctl stop waiver-rest-api
sudo systemctl stop waiver-oauth
sudo systemctl stop waiver-frontend
```

### Check Status
```bash
# Comprehensive health check
sudo /opt/waiver-exchange/health-check.sh

# Check individual service status
sudo systemctl status waiver-exchange
sudo systemctl status waiver-rest-api
sudo systemctl status waiver-oauth
sudo systemctl status waiver-frontend
```

### View Logs
```bash
# View logs for specific service
sudo /opt/waiver-exchange/view-logs.sh waiver-exchange
sudo /opt/waiver-exchange/view-logs.sh rest-api
sudo /opt/waiver-exchange/view-logs.sh oauth
sudo /opt/waiver-exchange/view-logs.sh frontend

# View all logs
sudo /opt/waiver-exchange/view-logs.sh all

# Or use journalctl directly
sudo journalctl -u waiver-exchange -f
sudo journalctl -u waiver-rest-api -f
sudo journalctl -u waiver-oauth -f
sudo journalctl -u waiver-frontend -f
```

## 🌐 Service Ports

| Service | Port | Description |
|---------|------|-------------|
| Nginx | 80, 443 | Reverse proxy (HTTP/HTTPS) |
| PostgreSQL | 5432 | Database |
| Redis | 6379 | Cache |
| Waiver Exchange | 8081 | WebSocket + OrderGateway |
| OAuth Server | 8082 | Authentication |
| REST API | 8083 | API endpoints |
| Frontend | 3000 | Next.js application |

## 📊 Monitoring

### Health Check
The health check script performs comprehensive checks:
- Service status (systemd)
- Port availability
- Database connectivity
- API endpoint responses
- System resources (CPU, memory, disk)
- Load average and uptime

### Log Files
All logs are stored in `/var/log/waiver-exchange/`:
- `waiver-exchange.log` - Main trading engine
- `rest-api.log` - REST API server
- `oauth.log` - OAuth authentication
- `frontend.log` - Frontend application

### Log Rotation
Logs are automatically rotated daily and kept for 30 days.

## 🔒 Security

### Firewall
UFW is configured with the following rules:
- SSH (port 22)
- HTTP (port 80)
- HTTPS (port 443)
- Application ports (8081, 8082, 8083, 3000)

### SSL/TLS
- Let's Encrypt certificates
- TLS 1.2 and 1.3 support
- Security headers configured
- HSTS enabled

### User Permissions
- Application runs as `waiver` user
- No shell access for application user
- Proper file permissions set

## 🚨 Troubleshooting

### Common Issues

#### Service Won't Start
```bash
# Check service status
sudo systemctl status [service-name]

# Check logs
sudo journalctl -u [service-name] -f

# Check configuration
sudo systemctl cat [service-name]
```

#### Port Already in Use
```bash
# Check what's using the port
sudo netstat -tlnp | grep :[port]

# Kill the process
sudo kill -9 [PID]
```

#### Database Connection Issues
```bash
# Check PostgreSQL status
sudo systemctl status postgresql

# Test connection
sudo -u postgres psql -d waiver_exchange -c "SELECT 1;"

# Check database logs
sudo journalctl -u postgresql -f
```

#### Redis Connection Issues
```bash
# Check Redis status
sudo systemctl status redis-server

# Test connection
redis-cli ping

# Check Redis logs
sudo journalctl -u redis-server -f
```

### Performance Issues

#### High CPU Usage
```bash
# Check top processes
top

# Check system resources
htop

# Check service-specific metrics
sudo systemctl status [service-name]
```

#### High Memory Usage
```bash
# Check memory usage
free -h

# Check memory by process
ps aux --sort=-%mem | head
```

#### High Disk Usage
```bash
# Check disk usage
df -h

# Check largest directories
du -sh /* | sort -hr
```

## 🔄 Updates and Maintenance

### Application Updates
```bash
# Stop services
sudo /opt/waiver-exchange/stop-all-services.sh

# Backup current binaries
sudo cp /opt/waiver-exchange/waiver-exchange /opt/waiver-exchange/waiver-exchange.backup
sudo cp /opt/waiver-exchange/rest-server /opt/waiver-exchange/rest-server.backup
sudo cp /opt/waiver-exchange/oauth-server /opt/waiver-exchange/oauth-server.backup

# Deploy new binaries
sudo cp target/release/waiver-exchange /opt/waiver-exchange/
sudo cp target/release/rest-server /opt/waiver-exchange/
sudo cp target/release/oauth-server /opt/waiver-exchange/

# Set ownership
sudo chown waiver:waiver /opt/waiver-exchange/*

# Start services
sudo /opt/waiver-exchange/start-all-services.sh
```

### System Updates
```bash
# Update system packages
sudo apt update && sudo apt upgrade -y

# Restart services if needed
sudo systemctl restart waiver-exchange waiver-rest-api waiver-oauth
```

### Database Backups
```bash
# Create backup
sudo -u postgres pg_dump waiver_exchange > backup_$(date +%Y%m%d_%H%M%S).sql

# Restore backup
sudo -u postgres psql waiver_exchange < backup_file.sql
```

## 📞 Support

### Logs and Diagnostics
```bash
# Full system health check
sudo /opt/waiver-exchange/health-check.sh

# System information
uname -a
lsb_release -a
systemctl --version
nginx -v
```

### Emergency Procedures
```bash
# Emergency stop all services
sudo systemctl stop waiver-exchange waiver-rest-api waiver-oauth waiver-frontend

# Emergency restart
sudo reboot

# Check system after restart
sudo /opt/waiver-exchange/health-check.sh
```

## 🎯 Production Checklist

- [ ] VPS provisioned with Ubuntu 22.04 LTS
- [ ] Domain DNS configured
- [ ] SSL certificate obtained
- [ ] All services running and healthy
- [ ] Firewall configured
- [ ] Log rotation set up
- [ ] Backup procedures in place
- [ ] Monitoring configured
- [ ] Health checks passing
- [ ] Performance benchmarks met

## 📈 Scaling

### Vertical Scaling
- Increase VPS resources (CPU, RAM, disk)
- Optimize application configuration
- Tune database parameters

### Horizontal Scaling
- Load balancer for multiple instances
- Database replication
- Redis clustering
- CDN for static assets

---

**Note**: This deployment is designed for production use with proper security, monitoring, and maintenance procedures. Always test changes in a staging environment before applying to production.
