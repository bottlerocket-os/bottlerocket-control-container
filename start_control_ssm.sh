#!/usr/bin/env bash

set -e

declare -r PERSISTENT_STORAGE_BASE_DIR="/.bottlerocket/host-containers/current"
declare -r USER_DATA="${PERSISTENT_STORAGE_BASE_DIR}/user-data"
declare -r SSM_AGENT_PERSISTENT_STATE_DIR="${PERSISTENT_STORAGE_BASE_DIR}/ssm"
declare -r SSM_AGENT_LOCAL_STATE_DIR="/var/lib/amazon/ssm"
declare -r HOST_CERTS="/.bottlerocket/certs"

#shellcheck disable=SC2155  # If not set then we'll treat it as 0
declare -r FIPS_MODE_FLAG=$(cat '/proc/sys/crypto/fips_enabled' 2>/dev/null || echo 0)

log() {
  echo "$*" >&2
}

# Link host certs if present into container & run update-ca-trust
link_host_certs() {
  for cert in $(ls -1 "${HOST_CERTS}"); do
    ln -s "${HOST_CERTS}/${cert}" "/etc/pki/ca-trust/source/anchors/${cert}"
  done
  # Update the CA trust to pickup the new certificates
  update-ca-trust
}

enable_hybrid_env_ssm() {
  # SSM parameters necessary to register with a hybrid activation
  local activation_code
  local activation_id
  local region

  # SC2155 suggests assigning values after declaration to preserve return codes
  activation_code=$(fetch_from_json '.["ssm"]["activation-code"]' "${USER_DATA}")
  activation_id=$(fetch_from_json '.["ssm"]["activation-id"]' "${USER_DATA}")
  region=$(fetch_from_json '.["ssm"]["region"]' "${USER_DATA}")

  # Register with AWS Systems Manager (SSM)
  if ! amazon-ssm-agent -register -code "${activation_code}" -id "${activation_id}" -region "${region}"; then

    # Print errors from ssm agent error log,
    # as they don't print to the EC2 console otherwise.
    cat "/var/log/amazon/ssm/errors.log" >&2

    log "Failed to register with AWS Systems Manager (SSM)"
    exit 1
  fi
}

# Fetch the values from json, and exit on failure (if any)
fetch_from_json() {
  local key="${1:?}"
  local file="${2:?}"
  local value
  if ! value=$(jq -e -r "${key}" "${file}"); then
    log "Unable to parse ${key} from ${file}"
    return 1
  fi
  if [[ -z "${value}" ]]; then
    log "No value set for ${key} in ${file}"
    return 1
  fi
  echo "${value}"
}

# If /.bottlerocket/host-containers/current/user-data exists and is not empty
# and the symlinked /var/lib/amazon/ssm/registration file is not populated,
# then check to see if the user-data file contains ssm at the top-level. If so,
# attempt to manually register with SSM with a hybrid activation.

if [[ ${FIPS_MODE_FLAG} -eq 1 ]]; then
  update-crypto-policies --set FIPS 2>/dev/null
  if [[ "$(cat '/etc/crypto-policies/config')" != "FIPS" ]]; then
    log "Failed to validate FIPS configuration"
    exit 1
  fi

  # Enable the Go Cryptographic Module to operate in FIPS 140-3 mode at runtime
  export GODEBUG='fips140=on'
fi

[[ -d "${HOST_CERTS}" ]] && link_host_certs

mkdir -p "${SSM_AGENT_PERSISTENT_STATE_DIR}"
chmod 750 "${SSM_AGENT_PERSISTENT_STATE_DIR}"

if [[ -s "${USER_DATA}" ]] \
&& [[ ! -s "${SSM_AGENT_LOCAL_STATE_DIR}/registration" ]] \
&& jq --exit-status '.ssm' "${USER_DATA}" &>/dev/null ; then
  enable_hybrid_env_ssm
fi

# Run Inspector SBOM upload; errors are logged but do not block container startup
if jq -e '.inspector["upload-sbom"] == false' "${USER_DATA}" &>/dev/null; then
  log "Inspector SBOM upload disabled via user-data"
else
  if [[ ${FIPS_MODE_FLAG} -eq 1 ]]; then
    /usr/sbin/corgid-fips || log "Inspector SBOM upload failed, continuing with container startup"
  else
    /usr/sbin/corgid || log "Inspector SBOM upload failed, continuing with container startup"
  fi
fi

# Start a single ssm process in the foreground
exec /usr/bin/amazon-ssm-agent
