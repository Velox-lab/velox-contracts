#!/usr/bin/env bash
# Deploy the full Velox contract set to a Stellar network, in the order
# described in ARCHITECTURE.md's "Deployment Order":
#
#   1. Deploy VeloxRegistry (constructor arg: admin address)
#   2. Upload PaymentStream WASM (used as template; note its hash)
#   3. Upload RecurringPayment WASM (used as template; note its hash)
#   4. Deploy StreamFactory, then call initialize(admin, registry, stream_hash, schedule_hash)
#   5. Registry admin calls VeloxRegistry.set_factory(factory_address)
#
# Usage:
#   scripts/deploy.sh <network> <source-identity>
#
#   network          testnet | futurenet | mainnet (anything `stellar network ls` knows)
#   source-identity  a `stellar keys` identity name. It pays the deploy fees and
#                    becomes the admin of both VeloxRegistry and StreamFactory.
#
# Example:
#   stellar keys generate velox-admin
#   stellar keys fund velox-admin --network testnet
#   scripts/deploy.sh testnet velox-admin
#
# Requires the Stellar CLI (https://developers.stellar.org/docs/tools/cli)
# and the release WASM built (`cargo build --locked --target wasm32v1-none --release`).
#
# Writes the deployed addresses to deployed.<network>.env on success, in the
# shape velox-scheduler's and velox-frontend's .env files expect.
set -euo pipefail

NETWORK="${1:?usage: scripts/deploy.sh <network> <source-identity>}"
SOURCE="${2:?usage: scripts/deploy.sh <network> <source-identity>}"

WASM_DIR="target/wasm32v1-none/release"
STREAM_WASM="$WASM_DIR/payment_stream.wasm"
SCHEDULE_WASM="$WASM_DIR/recurring_payment.wasm"
REGISTRY_WASM="$WASM_DIR/velox_registry.wasm"
FACTORY_WASM="$WASM_DIR/stream_factory.wasm"

for f in "$STREAM_WASM" "$SCHEDULE_WASM" "$REGISTRY_WASM" "$FACTORY_WASM"; do
  if [ ! -f "$f" ]; then
    echo "error: $f not found. Run: cargo build --locked --target wasm32v1-none --release" >&2
    exit 1
  fi
done

if ! command -v stellar >/dev/null 2>&1; then
  echo "error: stellar CLI not found. See https://developers.stellar.org/docs/tools/cli" >&2
  exit 1
fi

ADMIN=$(stellar keys public-key "$SOURCE")
echo "Deploying to $NETWORK as $SOURCE ($ADMIN)"

echo "==> Deploying VeloxRegistry"
REGISTRY_ID=$(stellar contract deploy \
  --wasm "$REGISTRY_WASM" \
  --source-account "$SOURCE" \
  --network "$NETWORK" \
  -- --admin "$ADMIN")
echo "    VeloxRegistry: $REGISTRY_ID"

echo "==> Uploading PaymentStream WASM"
STREAM_HASH=$(stellar contract upload \
  --wasm "$STREAM_WASM" \
  --source-account "$SOURCE" \
  --network "$NETWORK")
echo "    PaymentStream WASM hash: $STREAM_HASH"

echo "==> Uploading RecurringPayment WASM"
SCHEDULE_HASH=$(stellar contract upload \
  --wasm "$SCHEDULE_WASM" \
  --source-account "$SOURCE" \
  --network "$NETWORK")
echo "    RecurringPayment WASM hash: $SCHEDULE_HASH"

echo "==> Deploying StreamFactory"
FACTORY_ID=$(stellar contract deploy \
  --wasm "$FACTORY_WASM" \
  --source-account "$SOURCE" \
  --network "$NETWORK")
echo "    StreamFactory: $FACTORY_ID"

echo "==> Initializing StreamFactory"
stellar contract invoke \
  --contract-id "$FACTORY_ID" \
  --source-account "$SOURCE" \
  --network "$NETWORK" \
  -- initialize \
  --admin "$ADMIN" \
  --registry "$REGISTRY_ID" \
  --stream-wasm-hash "$STREAM_HASH" \
  --schedule-wasm-hash "$SCHEDULE_HASH"

echo "==> Linking VeloxRegistry to StreamFactory"
stellar contract invoke \
  --contract-id "$REGISTRY_ID" \
  --source-account "$SOURCE" \
  --network "$NETWORK" \
  -- set_factory \
  --factory "$FACTORY_ID"

OUT_FILE="deployed.$NETWORK.env"
cat > "$OUT_FILE" <<ENV
# Written by scripts/deploy.sh on $(date -u +%Y-%m-%dT%H:%M:%SZ)
STELLAR_NETWORK=$NETWORK
REGISTRY_CONTRACT_ID=$REGISTRY_ID
STREAM_FACTORY_CONTRACT_ID=$FACTORY_ID
PAYMENT_STREAM_WASM_HASH=$STREAM_HASH
RECURRING_PAYMENT_WASM_HASH=$SCHEDULE_HASH
ENV

echo
echo "Deployed to $NETWORK:"
echo "  VeloxRegistry:  $REGISTRY_ID"
echo "  StreamFactory:  $FACTORY_ID"
echo
echo "Addresses written to $OUT_FILE"
echo "REGISTRY_CONTRACT_ID from that file is what velox-scheduler's .env needs."
