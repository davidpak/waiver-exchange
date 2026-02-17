# Production Deployment Scripts

This directory contains deployment scripts for the Waiver Exchange platform.

## Linux Production Deployment

For production deployment, we use **Linux VPS with systemd services**. See the `linux/` directory for complete deployment scripts.

### Quick Start (Linux)
```bash
# Deploy to Ubuntu 22.04 LTS VPS
sudo ./scripts/linux/deploy.sh

# Start all services
sudo /opt/waiver-exchange/start-all-services.sh

# Health check
sudo /opt/waiver-exchange/health-check.sh
```

## Development Scripts

### Pre-commit Hook
- `pre-commit.sh` - Git pre-commit hook for code quality checks

## Service Architecture

### Backend Services
- **Waiver Exchange Service** (Port 8081) - WebSocket + OrderGateway
- **REST API Server** (Port 8083) - REST endpoints  
- **OAuth Server** (Port 8082) - Authentication
- **PostgreSQL** (Port 5432) - Database
- **Redis** (Port 6379) - Cache

### Frontend
- **Next.js Application** (Port 3000) - React frontend

## Production Deployment Options

### Option 1: Linux VPS (Recommended)
- **Cost**: $6-21/month
- **Setup**: Ubuntu 22.04 LTS with systemd
- **Scripts**: `linux/` directory
- **Features**: Auto-restart, logging, monitoring, SSL

### Option 2: Cloud Services
- **AWS/Google Cloud**: $90-165/month
- **Managed databases and services
- **High availability and scaling

## Documentation

- **Linux Deployment**: `linux/README.md`
- **Production Plan**: `docs/deployment/production_deployment_plan.md`
- **Architecture**: `docs/frontend/frontend_master.md`

## Support

For deployment issues:
1. Check the Linux deployment guide
2. Run health checks
3. Review service logs
4. Verify system requirements