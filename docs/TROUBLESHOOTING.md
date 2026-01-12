# LCARS Troubleshooting Guide

## Common Issues

### Authentication

#### "Unauthorized" error on all requests

**Symptoms:** API returns 401 for authenticated endpoints.

**Causes & Solutions:**

1. **Token expired** - JWT tokens expire after 24 hours. Log in again to get a new token.

2. **Invalid JWT secret** - If the server restarts with a different JWT secret, all existing tokens become invalid.
   - Solution: Set a fixed `LCARS_SERVER__JWT_SECRET` environment variable.

3. **Missing Authorization header** - Ensure requests include:
   ```
   Authorization: Bearer <your-token>
   ```

#### Forgot admin password

If you forget the admin password:

1. Stop the LCARS server
2. Delete the user from the database:
   ```bash
   sqlite3 /path/to/lcars.db "DELETE FROM users WHERE username='admin';"
   ```
3. Restart LCARS - a new admin user will be created with a random password (shown in logs)
4. Or set `LCARS_ADMIN_PASSWORD` environment variable before restarting

### Database

#### "Database is locked" errors

**Cause:** SQLite doesn't handle high concurrency well.

**Solutions:**

1. Ensure WAL mode is enabled (should be automatic):
   ```sql
   PRAGMA journal_mode = WAL;
   ```

2. Reduce concurrent operations if possible

3. Check for long-running queries or hung processes

#### Migration failed

**Symptoms:** Server fails to start with migration errors.

**Solutions:**

1. Check the error message for the specific migration that failed

2. Backup your database before manual fixes:
   ```bash
   cp lcars.db lcars.db.backup
   ```

3. For development, you can reset the database:
   ```bash
   rm lcars.db
   # Restart server - fresh database will be created
   ```

### Torrents

#### Downloads not starting

**Possible causes:**

1. **Torrent engine failed to initialize**
   - Check logs for torrent engine errors
   - Verify `torrent.download_dir` exists and is writable

2. **VPN interface not found**
   - If using `torrent.bind_interface`, ensure the interface exists:
     ```bash
     ip link show
     ```

3. **Port blocked by firewall**
   - Ensure ports 6881-6889 are accessible (or your configured range)

#### Slow download speeds

**Solutions:**

1. **Increase max connections:**
   ```toml
   [torrent]
   max_connections = 200
   ```

2. **Check VPN configuration** - Some VPNs throttle torrent traffic

3. **Verify port forwarding** - If behind NAT, forward the torrent ports

#### Downloads stuck at 99%

**Cause:** Usually the last pieces are rare.

**Solutions:**

1. Wait - it may complete eventually
2. Try a different release with more seeders
3. Check if the torrent is actually complete but marking failed

### Metadata

#### TMDB lookups failing

**Symptoms:** Movies/shows added without metadata.

**Solutions:**

1. **Check API key:**
   - Verify `LCARS_TMDB__API_KEY` is set correctly
   - Test the key at https://www.themoviedb.org/settings/api

2. **Rate limiting:**
   - TMDB has rate limits; wait and retry

3. **Network issues:**
   - Ensure the server can reach api.themoviedb.org

#### MusicBrainz errors

**Symptoms:** Artist/album lookups failing.

**Solutions:**

1. **Respect rate limits:**
   - MusicBrainz requires 1 second between requests
   - Increase `musicbrainz.rate_limit_ms` if needed

2. **Network issues:**
   - Ensure access to musicbrainz.org

### Storage

#### Files not being organized

**Symptoms:** Downloads complete but files stay in download directory.

**Causes:**

1. **Storage rules not configured:**
   ```toml
   [[storage.rules]]
   action = "move"
   destination = "media"
   media_types = ["movie", "tv", "music"]
   ```

2. **Mount not enabled or accessible:**
   - Check mount configuration
   - Verify permissions on destination directory

3. **Naming pattern error:**
   - Check logs for pattern substitution errors
   - Verify placeholders are valid

#### Permission denied errors

**Solutions:**

1. **Check file ownership:**
   ```bash
   ls -la /path/to/media
   ```

2. **Ensure LCARS user has write access:**
   ```bash
   chown -R lcars:lcars /path/to/media
   ```

3. **For Docker:**
   - Map volumes with correct UID/GID
   - Use `user: "1000:1000"` in docker-compose

### WebSocket

#### Real-time updates not working

**Symptoms:** Download progress not updating in UI.

**Solutions:**

1. **Check WebSocket connection:**
   - Open browser DevTools > Network > WS
   - Verify connection to `/api/ws`

2. **Token in query string:**
   - WebSocket uses `?token=` parameter
   - Token may be expired

3. **Proxy configuration:**
   - If behind nginx/caddy, ensure WebSocket upgrade is allowed:
     ```nginx
     location /api/ws {
         proxy_pass http://backend:8080;
         proxy_http_version 1.1;
         proxy_set_header Upgrade $http_upgrade;
         proxy_set_header Connection "upgrade";
     }
     ```

### Docker

#### Container won't start

**Check logs:**
```bash
docker logs lcars-backend
docker logs lcars-frontend
```

**Common issues:**

1. **Port already in use:**
   ```bash
   docker ps  # Check for conflicts
   lsof -i :8080  # Find process using port
   ```

2. **Volume permissions:**
   ```bash
   docker exec lcars-backend ls -la /data
   ```

3. **Missing environment variables:**
   - Ensure required vars are set in docker-compose.yml or .env file

#### Database corruption after crash

**Prevention:**
- Use named volumes instead of bind mounts for database
- Enable WAL mode (default)

**Recovery:**
```bash
# Backup corrupted database
cp lcars.db lcars.db.corrupt

# Try to recover
sqlite3 lcars.db.corrupt ".recover" | sqlite3 lcars.db.new

# Replace if recovery successful
mv lcars.db.new lcars.db
```

### Performance

#### High memory usage

**Possible causes:**

1. **Many concurrent downloads** - Reduce `torrent.max_connections`
2. **Large download queue** - Process fewer items simultaneously

#### Slow API responses

**Solutions:**

1. **Check database size:**
   ```bash
   ls -lh lcars.db
   ```

2. **Run VACUUM if large:**
   ```sql
   VACUUM;
   ```

3. **Check for missing indexes** - Update to latest version

## Getting Help

### Logs

Enable debug logging:
```bash
RUST_LOG=backend=debug,tower_http=debug
```

### Reporting Issues

When reporting issues, include:

1. LCARS version (`/health` endpoint)
2. Relevant log output
3. Configuration (redact secrets!)
4. Steps to reproduce

Report issues at: https://github.com/b4nst/lcars/issues
