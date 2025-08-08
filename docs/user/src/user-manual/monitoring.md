# Monitoring & Observability

Comprehensive monitoring is essential for operating Lloom nodes effectively. This guide covers setting up and using the monitoring stack, understanding metrics, and maintaining system health.

## Overview

The Lloom monitoring stack includes:
- **Prometheus**: Metrics collection and storage
- **Grafana**: Visualization and dashboards
- **AlertManager**: Alert routing and notifications
- **Node Exporter**: System metrics
- **Custom Exporters**: Lloom-specific metrics

## Quick Start

### Docker Compose Setup

1. **Clone the monitoring stack**:
   ```bash
   cd ~/lloom
   git clone https://github.com/dexloom/monitoring-stack
   cd monitoring-stack
   ```

2. **Start the stack**:
   ```bash
   docker-compose up -d
   ```

3. **Access dashboards**:
   - Grafana: http://localhost:3000 (admin/admin)
   - Prometheus: http://localhost:9090
   - AlertManager: http://localhost:9093

### Manual Setup

For production environments:

```bash
# Install Prometheus
wget https://github.com/prometheus/prometheus/releases/download/v2.45.0/prometheus-2.45.0.linux-amd64.tar.gz
tar xzf prometheus-2.45.0.linux-amd64.tar.gz
sudo mv prometheus-2.45.0.linux-amd64/prometheus /usr/local/bin/

# Install Grafana
sudo apt-get install -y software-properties-common
sudo add-apt-repository "deb https://packages.grafana.com/oss/deb stable main"
wget -q -O - https://packages.grafana.com/gpg.key | sudo apt-key add -
sudo apt-get update
sudo apt-get install grafana
```

## Prometheus Configuration

### Basic Configuration

Create `/etc/prometheus/prometheus.yml`:

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s
  external_labels:
    cluster: 'lloom-prod'
    region: 'us-east-1'

# Alerting
alerting:
  alertmanagers:
    - static_configs:
        - targets: ['localhost:9093']

# Rule files
rule_files:
  - 'alerts/*.yml'
  - 'recording_rules/*.yml'

# Scrape configurations
scrape_configs:
  # Lloom Client metrics
  - job_name: 'lloom-client'
    static_configs:
      - targets: ['client1:9091', 'client2:9091']
    relabel_configs:
      - source_labels: [__address__]
        target_label: instance
        regex: '([^:]+):.*'

  # Lloom Executor metrics
  - job_name: 'lloom-executor'
    static_configs:
      - targets: ['executor1:9092', 'executor2:9092']
    metrics_path: '/metrics'
    scrape_interval: 10s

  # Lloom Validator metrics
  - job_name: 'lloom-validator'
    static_configs:
      - targets: ['validator1:9093']

  # Node Exporter (system metrics)
  - job_name: 'node'
    static_configs:
      - targets: ['client1:9100', 'executor1:9100', 'validator1:9100']

  # NVIDIA GPU metrics
  - job_name: 'nvidia-gpu'
    static_configs:
      - targets: ['executor1:9835']

  # Ethereum node metrics
  - job_name: 'ethereum'
    static_configs:
      - targets: ['eth-node:6060']
```

### Service Discovery

For dynamic environments:

```yaml
scrape_configs:
  # Kubernetes service discovery
  - job_name: 'lloom-k8s'
    kubernetes_sd_configs:
      - role: pod
        namespaces:
          names: ['lloom']
    relabel_configs:
      - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_scrape]
        action: keep
        regex: true
      - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_port]
        action: replace
        target_label: __address__
        regex: ([^:]+)(?::\d+)?;(\d+)
        replacement: $1:$2

  # Consul service discovery
  - job_name: 'lloom-consul'
    consul_sd_configs:
      - server: 'consul:8500'
        services: ['lloom-client', 'lloom-executor', 'lloom-validator']
```

## Grafana Dashboards

### Import Dashboards

Pre-built dashboards are available:

1. **Lloom Overview Dashboard**:
   ```bash
   curl -X POST http://admin:admin@localhost:3000/api/dashboards/import \
     -H "Content-Type: application/json" \
     -d @dashboards/lloom-overview.json
   ```

2. **Executor Performance Dashboard**:
   - Model performance metrics
   - Token throughput
   - Revenue tracking
   - Resource utilization

3. **Network Health Dashboard**:
   - P2P network statistics
   - Peer connections
   - Message rates
   - DHT health

### Custom Dashboard Examples

#### Request Flow Dashboard

```json
{
  "dashboard": {
    "title": "Lloom Request Flow",
    "panels": [
      {
        "title": "Request Rate",
        "targets": [
          {
            "expr": "rate(lloom_client_requests_total[5m])",
            "legendFormat": "{{instance}} - {{status}}"
          }
        ],
        "type": "graph"
      },
      {
        "title": "Request Latency",
        "targets": [
          {
            "expr": "histogram_quantile(0.99, rate(lloom_request_duration_seconds_bucket[5m]))",
            "legendFormat": "p99 latency"
          }
        ],
        "type": "graph"
      }
    ]
  }
}
```

#### Cost Analysis Dashboard

```json
{
  "panels": [
    {
      "title": "Cumulative Costs",
      "targets": [
        {
          "expr": "lloom_client_cost_total",
          "legendFormat": "Client: {{instance}}"
        },
        {
          "expr": "lloom_executor_revenue_total",
          "legendFormat": "Executor: {{instance}}"
        }
      ]
    },
    {
      "title": "Cost per Token",
      "targets": [
        {
          "expr": "rate(lloom_client_cost_total[5m]) / rate(lloom_client_tokens_used_total[5m])",
          "legendFormat": "{{model}}"
        }
      ]
    }
  ]
}
```

## Key Metrics

### Client Metrics

```prometheus
# Request metrics
lloom_client_requests_total{status="success|failed|timeout"}
lloom_client_request_duration_seconds{quantile="0.5|0.9|0.99"}
lloom_client_active_requests

# Token metrics
lloom_client_tokens_used_total{type="inbound|outbound"}
lloom_client_tokens_per_request{model="..."}

# Cost metrics
lloom_client_cost_total{currency="ETH"}
lloom_client_cost_per_request{model="..."}

# Network metrics
lloom_client_peers_connected
lloom_client_executors_available{model="..."}
lloom_client_network_messages_total{type="sent|received"}
```

### Executor Metrics

```prometheus
# Request processing
lloom_executor_requests_total{model="...", status="..."}
lloom_executor_request_duration_seconds
lloom_executor_queue_size{priority="low|normal|high"}
lloom_executor_active_requests{model="..."}

# Model metrics
lloom_executor_model_load_time_seconds{model="..."}
lloom_executor_tokens_per_second{model="..."}
lloom_executor_model_errors_total{model="...", error="..."}

# Revenue metrics
lloom_executor_revenue_total{currency="ETH"}
lloom_executor_revenue_per_request{model="..."}

# Resource metrics
lloom_executor_gpu_utilization_percent{device="0|1|2|3"}
lloom_executor_gpu_memory_used_bytes{device="..."}
lloom_executor_cpu_usage_percent
lloom_executor_memory_usage_bytes
```

### Validator Metrics

```prometheus
# Validation metrics
lloom_validator_validations_total{result="pass|fail"}
lloom_validator_validation_duration_seconds
lloom_validator_violations_detected_total{type="..."}

# Network observation
lloom_validator_transactions_observed_total
lloom_validator_transactions_validated_total
lloom_validator_validation_sampling_rate

# Economic metrics
lloom_validator_rewards_earned_total{currency="ETH"}
lloom_validator_penalties_total{reason="..."}
lloom_validator_stake_amount{currency="ETH"}
```

### System Metrics

```prometheus
# CPU metrics
node_cpu_seconds_total{mode="idle|user|system"}
node_load1
node_load5
node_load15

# Memory metrics
node_memory_MemAvailable_bytes
node_memory_MemTotal_bytes
node_memory_SwapTotal_bytes

# Disk metrics
node_filesystem_avail_bytes{mountpoint="/"}
node_disk_io_time_seconds_total{device="..."}
node_disk_read_bytes_total{device="..."}
node_disk_written_bytes_total{device="..."}

# Network metrics
node_network_receive_bytes_total{device="..."}
node_network_transmit_bytes_total{device="..."}
node_network_receive_errs_total{device="..."}
```

## Alerting

### Alert Rules

Create `/etc/prometheus/alerts/lloom.yml`:

```yaml
groups:
  - name: lloom_client_alerts
    interval: 30s
    rules:
      - alert: HighClientErrorRate
        expr: |
          (
            rate(lloom_client_requests_total{status="failed"}[5m]) /
            rate(lloom_client_requests_total[5m])
          ) > 0.05
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High client error rate on {{ $labels.instance }}"
          description: "Error rate is {{ $value | humanizePercentage }} over the last 5 minutes"

      - alert: ClientRequestTimeout
        expr: |
          rate(lloom_client_requests_total{status="timeout"}[5m]) > 0.1
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Client experiencing timeouts"
          description: "{{ $labels.instance }} has {{ $value }} timeouts per second"

  - name: lloom_executor_alerts
    rules:
      - alert: ExecutorHighQueueSize
        expr: lloom_executor_queue_size > 100
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Executor queue backing up"
          description: "Queue size is {{ $value }} on {{ $labels.instance }}"

      - alert: ExecutorGPUMemoryHigh
        expr: |
          (
            lloom_executor_gpu_memory_used_bytes /
            lloom_executor_gpu_memory_total_bytes
          ) > 0.9
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "GPU memory critically high"
          description: "GPU {{ $labels.device }} on {{ $labels.instance }} at {{ $value | humanizePercentage }} capacity"

      - alert: ExecutorModelLoadFailure
        expr: increase(lloom_executor_model_errors_total{error="load_failed"}[5m]) > 0
        labels:
          severity: critical
        annotations:
          summary: "Model load failure"
          description: "Failed to load {{ $labels.model }} on {{ $labels.instance }}"

  - name: lloom_validator_alerts
    rules:
      - alert: ValidatorHighViolationRate
        expr: |
          (
            rate(lloom_validator_violations_detected_total[1h]) /
            rate(lloom_validator_validations_total[1h])
          ) > 0.05
        for: 15m
        labels:
          severity: critical
        annotations:
          summary: "High violation rate detected"
          description: "Violation rate is {{ $value | humanizePercentage }} over the last hour"

      - alert: ValidatorStakeAtRisk
        expr: lloom_validator_penalties_total > (lloom_validator_rewards_earned_total * 0.1)
        labels:
          severity: warning
        annotations:
          summary: "Validator penalties exceeding safe threshold"
          description: "Penalties are {{ $value }} ETH, which is more than 10% of rewards"
```

### AlertManager Configuration

Configure `/etc/alertmanager/alertmanager.yml`:

```yaml
global:
  resolve_timeout: 5m
  smtp_from: 'alerts@lloom.network'
  smtp_smarthost: 'smtp.gmail.com:587'
  smtp_auth_username: 'alerts@lloom.network'
  smtp_auth_password: 'your-password'

route:
  group_by: ['alertname', 'cluster', 'severity']
  group_wait: 10s
  group_interval: 10s
  repeat_interval: 12h
  receiver: 'default'
  routes:
    - match:
        severity: critical
      receiver: 'pagerduty'
      continue: true
    - match:
        severity: warning
      receiver: 'slack'

receivers:
  - name: 'default'
    email_configs:
      - to: 'ops@lloom.network'
        headers:
          Subject: 'Lloom Alert: {{ .GroupLabels.alertname }}'

  - name: 'pagerduty'
    pagerduty_configs:
      - service_key: 'your-pagerduty-key'
        description: '{{ .GroupLabels.alertname }}: {{ .CommonAnnotations.summary }}'

  - name: 'slack'
    slack_configs:
      - api_url: 'https://hooks.slack.com/services/YOUR/SLACK/WEBHOOK'
        channel: '#lloom-alerts'
        title: 'Lloom Alert'
        text: '{{ range .Alerts }}{{ .Annotations.description }}{{ end }}'
```

## Log Aggregation

### Loki Setup

For log aggregation with Grafana:

```yaml
# docker-compose.yml addition
loki:
  image: grafana/loki:2.9.0
  ports:
    - "3100:3100"
  volumes:
    - ./loki-config.yaml:/etc/loki/local-config.yaml
    - loki-data:/loki

promtail:
  image: grafana/promtail:2.9.0
  volumes:
    - /var/log:/var/log:ro
    - ./promtail-config.yaml:/etc/promtail/config.yml
    - /var/lib/docker/containers:/var/lib/docker/containers:ro
```

### Promtail Configuration

```yaml
server:
  http_listen_port: 9080

positions:
  filename: /tmp/positions.yaml

clients:
  - url: http://loki:3100/loki/api/v1/push

scrape_configs:
  - job_name: lloom_logs
    static_configs:
      - targets:
          - localhost
        labels:
          job: lloom
          __path__: /var/log/lloom/*.log
    pipeline_stages:
      - regex:
          expression: '^(?P<timestamp>\S+)\s+(?P<level>\S+)\s+(?P<component>\S+)\s+(?P<message>.*)$'
      - timestamp:
          source: timestamp
          format: RFC3339
      - labels:
          level:
          component:
```

## Performance Monitoring

### Request Tracing

Enable distributed tracing:

```toml
# In lloom configuration
[tracing]
enabled = true
backend = "jaeger"
endpoint = "http://jaeger:14268/api/traces"
sampling_rate = 0.1  # Sample 10% of requests
```

### Trace Analysis

Key traces to monitor:
- Request lifecycle (client → executor → response)
- Model loading and inference
- Network message propagation
- Validation process

### Performance Dashboards

Create SLO dashboards:

```prometheus
# Request success rate (SLO: 99.9%)
(
  sum(rate(lloom_client_requests_total{status="success"}[5m])) /
  sum(rate(lloom_client_requests_total[5m]))
) * 100

# Request latency (SLO: p99 < 5s)
histogram_quantile(0.99,
  sum(rate(lloom_request_duration_seconds_bucket[5m])) by (le)
)

# Availability (SLO: 99.95%)
avg_over_time(up{job=~"lloom-.*"}[5m]) * 100
```

## Capacity Planning

### Metrics for Scaling

Monitor these metrics for capacity planning:

```prometheus
# Request queue depth
lloom_executor_queue_size

# Resource utilization
(
  avg(lloom_executor_cpu_usage_percent) by (instance)
) > 80

# Token throughput vs capacity
rate(lloom_executor_tokens_processed_total[5m]) /
lloom_executor_max_tokens_per_second

# Memory pressure
(
  node_memory_MemAvailable_bytes /
  node_memory_MemTotal_bytes
) < 0.1
```

### Forecasting

Use Prometheus recording rules for trends:

```yaml
groups:
  - name: capacity_planning
    interval: 5m
    rules:
      - record: lloom:request_rate:5m
        expr: |
          sum(rate(lloom_client_requests_total[5m])) by (model)

      - record: lloom:token_usage_trend:1h
        expr: |
          predict_linear(lloom_executor_tokens_processed_total[1h], 3600)

      - record: lloom:revenue_projection:24h
        expr: |
          predict_linear(lloom_executor_revenue_total[24h], 86400)
```

## Best Practices

### Monitoring Strategy

1. **Golden Signals**:
   - Latency: Request duration percentiles
   - Traffic: Request rate
   - Errors: Error rate and types
   - Saturation: Resource utilization

2. **SLI/SLO Definition**:
   - Define Service Level Indicators
   - Set realistic Service Level Objectives
   - Create error budgets

3. **Alert Fatigue Prevention**:
   - Alert on symptoms, not causes
   - Use appropriate thresholds
   - Group related alerts

### Dashboard Design

1. **Overview First**:
   - Start with high-level health
   - Drill down to specifics
   - Use consistent color coding

2. **Time Windows**:
   - Default to reasonable ranges
   - Allow easy time selection
   - Include comparison periods

3. **Actionable Information**:
   - Include remediation hints
   - Link to runbooks
   - Show historical context

### Maintenance

1. **Regular Reviews**:
   - Review alert effectiveness monthly
   - Update thresholds based on baselines
   - Remove obsolete metrics

2. **Backup and Recovery**:
   ```bash
   # Backup Prometheus data
   tar -czf prometheus-backup-$(date +%Y%m%d).tar.gz /var/lib/prometheus

   # Backup Grafana dashboards
   for dash in $(curl -s http://admin:admin@localhost:3000/api/search | jq -r '.[].uid'); do
     curl -s http://admin:admin@localhost:3000/api/dashboards/uid/$dash | 
     jq -r '.dashboard' > dashboard-$dash.json
   done
   ```

3. **Retention Policies**:
   ```yaml
   # prometheus.yml
   global:
     external_labels:
       cluster: 'prod'
   
   # Storage retention
   storage:
     tsdb:
       retention.time: 30d
       retention.size: 100GB
   ```

## Troubleshooting Monitoring

### Common Issues

1. **Missing Metrics**:
   - Check target is up in Prometheus
   - Verify firewall rules
   - Check metric path and port

2. **High Cardinality**:
   - Review label usage
   - Implement metric relabeling
   - Use recording rules

3. **Slow Queries**:
   - Optimize PromQL expressions
   - Use recording rules for complex queries
   - Increase Prometheus resources

### Debug Commands

```bash
# Check Prometheus targets
curl http://localhost:9090/api/v1/targets

# Test metric endpoint
curl http://executor:9092/metrics | grep lloom_

# Check Grafana datasources
curl http://admin:admin@localhost:3000/api/datasources

# Validate alert rules
promtool check rules /etc/prometheus/alerts/*.yml

# Test alertmanager config
amtool check-config /etc/alertmanager/alertmanager.yml
```