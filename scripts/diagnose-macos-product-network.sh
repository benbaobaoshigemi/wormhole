#!/bin/bash
# macOS Network Diagnosis for Wormhole Product

echo "=========================================="
echo "      Wormhole Mac Network Diagnosis      "
echo "=========================================="

# 1. 当前 wormhole-daemon 进程路径和 command line
echo -e "\n[1/7] Running wormhole-daemon processes:"
ps_output=$(ps aux | grep wormhole-daemon | grep -v grep)
if [ -n "$ps_output" ]; then
  echo "$ps_output"
else
  echo "No running wormhole-daemon processes found."
fi

# 2. config path, bind_host, local port, peer host, peer port
echo -e "\n[2/7] Configuration settings:"
# 尝试找到配置文件的路径并读取
# config.json 默认在 daemon 所在目录下的 config/config.json 或 ~/.wormhole/config.json
config_path=""
if [ -f "$HOME/.wormhole/config.json" ]; then
  config_path="$HOME/.wormhole/config.json"
elif [ -d "$HOME/Desktop/hole" ]; then
  # 寻找开发测试或打包目录下的 config
  if [ -f "$HOME/Desktop/hole/target/product/macos/Wormhole/config/config.json" ]; then
    config_path="$HOME/Desktop/hole/target/product/macos/Wormhole/config/config.json"
  fi
fi

# 如果还是没找到，尝试在当前目录搜索
if [ -z "$config_path" ] && [ -f "./config/config.json" ]; then
  config_path="./config/config.json"
fi

if [ -n "$config_path" ] && [ -f "$config_path" ]; then
  echo "Found config path: $config_path"
  bind_host=$(grep -o '"bind_host": "[^"]*' "$config_path" | cut -d'"' -f4)
  port=$(grep -o '"port": [0-9]*' "$config_path" | grep -o '[0-9]*')
  peer_host=$(grep -o '"host": "[^"]*' "$config_path" | tail -n 1 | cut -d'"' -f4)
  peer_port=$(grep -o '"port": [0-9]*' "$config_path" | tail -n 1 | grep -o '[0-9]*')
  
  echo "Bind Host   : $bind_host"
  echo "Local Port  : $port"
  echo "Peer Host   : $peer_host"
  echo "Peer Port   : $peer_port"
else
  echo "Could not locate a config.json file automatically."
  port="${WORMHOLE_LOCAL_PORT:-}"
  peer_host="${WORMHOLE_PEER_HOST:-}"
  peer_port="${WORMHOLE_PEER_PORT:-}"
fi

# 3. daemon 监听的端口
echo -e "\n[3/7] Netstat Port Listening Status:"
if [ -n "$port" ] && command -v lsof >/dev/null 2>&1; then
  lsof -i :"$port" | grep LISTEN
elif [ -n "$port" ]; then
  netstat -an | grep "$port" | grep LISTEN
else
  echo "Local port is unknown because no readable config was found."
fi

# 4. Local API State Test (curl http://127.0.0.1:<local_port>/local/state)
echo -e "\n[4/7] Local API State Test (http://127.0.0.1:$port/local/state):"
if [ -n "$port" ]; then
  curl -s -v "http://127.0.0.1:$port/local/state" 2>&1
else
  echo "Skipped because local port is unknown."
fi

# 5. Peer Handshake Test (curl http://<peer_host>:<peer_port>/peer/handshake)
echo -e "\n[5/7] Peer Handshake Test (http://$peer_host:$peer_port/peer/handshake):"
if [ -n "$peer_host" ] && [ -n "$peer_port" ]; then
  handshake_output=$(curl --max-time 5 -s -w "\nHTTP_STATUS:%{http_code}\n" "http://$peer_host:$peer_port/peer/handshake")
  echo "$handshake_output"
  
  http_code=$(echo "$handshake_output" | grep "HTTP_STATUS" | cut -d':' -f2)
  if [ "$http_code" != "200" ]; then
    echo -e "\n[ERROR] Peer handshake failed with status code: $http_code"
    echo "This indicates the Windows peer daemon is offline, ports are mismatched, or blocked by Windows Firewall."
  else
    echo -e "\n[SUCCESS] Successfully handshaked with Windows peer!"
  fi
else
  echo "Peer host or port is not configured, skipping handshake test."
fi

# 6. 本机 LAN IP
echo -e "\n[6/7] Local IP Addresses:"
ifconfig | grep "inet " | grep -v 127.0.0.1

echo "=========================================="
echo "          Diagnosis Complete              "
echo "=========================================="
