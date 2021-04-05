#!/usr/bin/env bash

set -e

declare -r PERSISTENT_STORAGE_BASE_DIR="/.bottlerocket/host-containers/current"
declare -r USER_DATA="${PERSISTENT_STORAGE_BASE_DIR}/user-data"
declare -r SSM_AGENT_PERSISTENT_STATE_DIR="${PERSISTENT_STORAGE_BASE_DIR}/ssm"
declare -r SSM_AGENT_LOCAL_STATE_DIR="/var/lib/amazon/ssm"

log() {
  echo "$*" >&2
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

mkdir -p "${SSM_AGENT_PERSISTENT_STATE_DIR}"
chmod 750 "${SSM_AGENT_PERSISTENT_STATE_DIR}"

if [[ -s "${USER_DATA}" ]] \
&& [[ ! -s "${SSM_AGENT_LOCAL_STATE_DIR}/registration" ]] \
&& jq --exit-status '.ssm' "${USER_DATA}" &>/dev/null ; then
  enable_hybrid_env_ssm
fi

# Start a single ssm process in the foreground
exec /usr/bin/amazon-ssm-agent
