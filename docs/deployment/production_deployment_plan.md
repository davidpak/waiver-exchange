# Production Deployment Plan for waiver.exchange

## Overview
This document outlines the complete production deployment strategy for the Waiver Exchange trading platform, covering both self-hosting and cloud deployment options.

## Current Architecture

### Services Running:
1. **Waiver Exchange Service** - Port 8081 (WebSocket + OrderGateway)
2. **REST API Server** - Port 8083 (`rest_server` binary)
3. **OAuth Server** - Port 8082 (`oauth-server` binary) 
4. **PostgreSQL Database** - Port 5432
5. **Redis Cache** - Port 6379
6. **Frontend** - Port 3000 (Next.js)

### Current Status:
- ✅ All services working correctly on separate ports
- ✅ Port conflicts resolved (REST API moved to 8083)
- ✅ Environment variables properly configured
- ✅ Database migrations working
- ✅ Ready for production deployment

## Deployment Options

### Option 1: Linux VPS with systemd (Recommended for Production)
**Pros:**
- Production-ready and reliable
- Better performance than Docker
- Native process management with systemd
- Lower resource usage
- Industry standard for production servers
- Easy scaling and monitoring

**Cons:**
- Requires Linux server management knowledge
- Manual service configuration

**Implementation:**
- Ubuntu 22.04 LTS VPS
- systemd services for auto-start and management
- Nginx reverse proxy
- Let's Encrypt SSL certificates
- Bash scripts for deployment and management

### Option 2: Docker Compose (Development/Testing Only)
**Pros:**
- Easy local development
- Service isolation
- Simple configuration
- No code changes required

**Cons:**
- Multiple containers to manage
- Resource overhead
- Not ideal for production scaling
- Docker Desktop issues on Windows

**Implementation:**
```yaml
version: '3.8'
services:
  postgres:
    image: postgres:15
    environment:
      POSTGRES_DB: waiver_exchange
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: password
    ports:
      - "5432:5432"
    volumes:
      - postgres_data:/var/lib/postgresql/data

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"

  waiver-exchange:
    build: .
    ports:
      - "8081:8081"  # WebSocket
      - "8082:8082"  # OAuth
      - "8083:8083"  # REST API
    depends_on:
      - postgres
      - redis
    environment:
      DATABASE_URL: postgresql://postgres:password@postgres:5432/waiver_exchange
      REDIS_URL: redis://redis:6379

volumes:
  postgres_data:
```

### Option 3: Self-Hosting (Budget-Friendly)

#### Home Server Setup:
**Hardware Requirements:**
- CPU: 4+ cores (Intel i5/AMD Ryzen 5 or better)
- RAM: 8GB+ (16GB recommended)
- Storage: 100GB+ SSD
- Network: Stable internet connection (10+ Mbps upload)

**Software Stack:**
- Ubuntu Server 22.04 LTS
- systemd services
- Nginx (reverse proxy)
- Let's Encrypt (SSL certificates)
- UFW (firewall)

**Monthly Costs:**
- Electricity: ~$10-20
- Internet: Existing connection
- Domain: $10-15/year
- **Total: ~$10-30/month**

#### VPS Setup (Recommended for Production):
**Provider Options:**
- DigitalOcean: $5-20/month
- Linode: $5-20/month
- Vultr: $6-20/month
- Hetzner: €4-15/month

**Recommended Specifications:**
- 2 vCPUs
- 4GB RAM
- 50GB SSD
- 2TB+ bandwidth
- Ubuntu 22.04 LTS

### Option 4: Cloud Deployment (Enterprise)

#### AWS Setup:
**Services:**
- EC2 (t3.medium): $30-50/month
- RDS PostgreSQL: $25-50/month
- ElastiCache Redis: $15-30/month
- Application Load Balancer: $20-30/month
- Route 53: $1-5/month
- **Total: $90-165/month**

#### Google Cloud Setup:
**Services:**
- Compute Engine: $25-40/month
- Cloud SQL: $25-50/month
- Memorystore Redis: $15-25/month
- Load Balancer: $20-30/month
- **Total: $85-145/month**

## Production Deployment Steps

### Phase 1: Infrastructure Setup
1. **Choose deployment option** (Linux VPS recommended)
2. **Set up Ubuntu 22.04 LTS server**
3. **Install system dependencies** (Rust, PostgreSQL, Redis, Nginx)
4. **Configure firewall** (UFW)
5. **Set up domain DNS** (waiver.exchange)

### Phase 2: Service Deployment
1. **Deploy database** (PostgreSQL with systemd)
2. **Deploy cache** (Redis with systemd)
3. **Build and deploy application** (Waiver Exchange services)
4. **Configure systemd services** (auto-start and management)
5. **Configure reverse proxy** (Nginx)
6. **Set up SSL certificates** (Let's Encrypt)

### Phase 3: Monitoring & Security
1. **Set up monitoring** (Prometheus + Grafana)
2. **Configure logging** (ELK Stack or similar)
3. **Set up backups** (automated database backups)
4. **Configure alerts** (uptime monitoring)
5. **Security hardening** (fail2ban, SSH keys)

### Phase 4: CI/CD Pipeline
1. **Set up Git repository** (GitHub/GitLab)
2. **Configure automated testing**
3. **Set up deployment pipeline**
4. **Configure staging environment**
5. **Set up rollback procedures**

## Security Considerations

### Essential Security Measures:
1. **SSL/TLS encryption** (Let's Encrypt)
2. **Firewall configuration** (UFW/iptables)
3. **Regular security updates**
4. **Database encryption at rest**
5. **API rate limiting**
6. **Input validation and sanitization**
7. **Secure authentication** (OAuth + JWT)
8. **Regular backups** (encrypted)

### Trading-Specific Security:
1. **Financial data encryption**
2. **Audit logging** (all trades/transactions)
3. **Compliance monitoring**
4. **Risk management systems**
5. **Fraud detection**

## Monitoring & Maintenance

### Key Metrics to Monitor:
1. **System Performance:**
   - CPU usage
   - Memory usage
   - Disk space
   - Network latency

2. **Application Metrics:**
   - API response times
   - Error rates
   - Database performance
   - Cache hit rates

3. **Business Metrics:**
   - Active users
   - Trade volume
   - Revenue metrics
   - User engagement

### Maintenance Schedule:
- **Daily:** Health checks, log monitoring
- **Weekly:** Security updates, performance review
- **Monthly:** Backup verification, capacity planning
- **Quarterly:** Security audit, disaster recovery testing

## Cost Breakdown

### Self-Hosting (VPS):
- VPS: $5-20/month
- Domain: $1/month
- SSL: Free (Let's Encrypt)
- Monitoring: Free (open source tools)
- **Total: $6-21/month**

### Cloud Deployment:
- Infrastructure: $90-165/month
- Monitoring: $20-50/month
- Backup storage: $10-20/month
- **Total: $120-235/month**

## Rollback Plan

### Immediate Rollback (5 minutes):
1. Stop new deployment
2. Restart previous version
3. Verify service health
4. Notify users if needed

### Database Rollback:
1. Restore from latest backup
2. Replay transaction logs
3. Verify data integrity
4. Update application if needed

### Full Disaster Recovery:
1. Provision new infrastructure
2. Restore from backups
3. Update DNS records
4. Verify all services
5. **Recovery Time: 1-4 hours**

## Next Steps

1. **Create Linux deployment scripts** (bash scripts for service management)
2. **Set up systemd service files** (auto-start and process management)
3. **Create Nginx configuration** (reverse proxy setup)
4. **Test on local Linux VM** (Ubuntu 22.04)
5. **Deploy to production VPS** (DigitalOcean/Linode/Vultr)

## Linux Deployment Scripts

### Required Scripts:
- `deploy.sh` - Main deployment script
- `start-all-services.sh` - Start all services
- `stop-all-services.sh` - Stop all services
- `health-check.sh` - Service health monitoring
- `view-logs.sh` - View service logs
- `setup-production-env.sh` - Environment setup

### systemd Service Files:
- `waiver-exchange.service` - Main trading engine
- `rest-api.service` - REST API server
- `oauth-server.service` - OAuth authentication
- `postgresql.service` - Database (system package)
- `redis.service` - Cache (system package)

### Nginx Configuration:
- Reverse proxy for all services
- SSL termination with Let's Encrypt
- Load balancing (if needed)
- Static file serving for frontend

## Conclusion

For production deployment, **Linux VPS with systemd** is recommended. This provides:
- Low cost ($6-21/month)
- Production-grade reliability
- Native process management
- Industry-standard deployment
- Easy scaling and monitoring

The deployment plan prioritizes security, reliability, and cost-effectiveness while maintaining the ability to scale as the platform grows.
