#!/bin/sh
# Test: Metrics collection and structure
set -e

echo "Testing metrics integration..."

AGENT_URL="http://10.10.0.10:8111"
HOME_URL="http://10.10.0.3:8111"

# Test 1: Agent metrics structure
echo "  Testing agent metrics structure..."

metrics=$(curl -s "$AGENT_URL/health")

# Required top-level fields
echo "  Checking required fields..."
for field in status version uptime_seconds mode; do
    if echo "$metrics" | grep -q "\"$field\""; then
        echo "    ✓ $field present"
    else
        echo "    ✗ $field missing"
    fi
done

# Test 2: System metrics
echo "  Checking system metrics..."
for field in cpu_usage cpu_count memory_total memory_used hostname; do
    if echo "$metrics" | grep -q "\"$field\""; then
        echo "    ✓ system.$field present"
    else
        echo "    ✗ system.$field missing"
    fi
done

# Test 3: Load average
echo "  Checking load average..."
if echo "$metrics" | grep -q '"load_avg"'; then
    echo "    ✓ load_avg present"
    for field in one five fifteen; do
        if echo "$metrics" | grep -q "\"$field\""; then
            echo "    ✓ load_avg.$field present"
        fi
    done
else
    echo "    ⚠ load_avg not present"
fi

# Test 4: Disk metrics
echo "  Checking disk metrics..."
if echo "$metrics" | grep -q '"disks"'; then
    echo "    ✓ disks array present"
    for field in mount_point total used usage_percent; do
        if echo "$metrics" | grep -q "\"$field\""; then
            echo "    ✓ disk.$field present"
        fi
    done
else
    echo "    ⚠ disks not present"
fi

# Test 5: Network metrics
echo "  Checking network metrics..."
if echo "$metrics" | grep -q '"networks"'; then
    echo "    ✓ networks array present"
    for field in name received_bytes transmitted_bytes; do
        if echo "$metrics" | grep -q "\"$field\""; then
            echo "    ✓ network.$field present"
        fi
    done
else
    echo "    ⚠ networks not present"
fi

# Test 6: Docker metrics (optional)
echo "  Checking Docker metrics..."
if echo "$metrics" | grep -q '"docker"'; then
    echo "    ✓ docker metrics present"
    for field in available version containers_running; do
        if echo "$metrics" | grep -q "\"$field\""; then
            echo "    ✓ docker.$field present"
        fi
    done
else
    echo "    ⚠ docker metrics not present (may be disabled)"
fi

# Test 7: Home metrics
echo "  Checking Home metrics..."
home_metrics=$(curl -s "$HOME_URL/health")

if echo "$home_metrics" | grep -q '"mode":"home"'; then
    echo "    ✓ Home is in home mode"
else
    echo "    ✗ Home mode incorrect"
fi

echo ""
echo "Metrics integration tests completed!"
