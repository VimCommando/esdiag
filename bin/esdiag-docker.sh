#!/bin/bash

# Export these variables, they will be passed to the container
declare ESDIAG_OUTPUT_URL=${ESDIAG_OUTPUT_URL:-"http://host.docker.internal:9200"}
declare ESDIAG_OUTPUT_APIKEY=${ESDIAG_OUTPUT_APIKEY}
declare ESDIAG_OUTPUT_USERNAME=${ESDIAG_OUTPUT_USERNAME}
declare ESDIAG_OUTPUT_PASSWORD=${ESDIAG_OUTPUT_PASSWORD}

function validate_image() {
    # call docker inspect esdiag:latest, with jq check that .[].RepoTags[0] == "esdiag:latest"
    if ! command -v jq &> /dev/null; then
        echo "'jq' is required to validate the container image"
        echo "Check your distribution or homebrew package manager"
        exit 1
    fi
    local is_valid=$(docker inspect esdiag:latest | jq '.[].RepoTags[0] == "esdiag:latest"')
    if [[ "${is_valid}" != "true" ]]; then
        echo "NO container image found with tag: esdiag:latest"
        echo "From the repository root, please run 'docker build --tag esdiag:latest .'"
        exit 1
    else
        echo "Container image found with tag: esdiag:latest"
    fi
}

# If diag_path is a local file or directory, mount it to the container
function docker_run() {
    declare input="${1}"; shift
    if [[ -f "${input}" ]] || [[ -d "${input}" ]]; then
        echo "Path ${input} is local file or directory, mounting to container"
        declare diag_mount="/data/diagnostic"
    fi

    echo "Running esdiag ${command} ${input} ${*}"

    docker run --rm ${diag_mount:+--volume ${input}:${diag_mount}} \
        --env ESDIAG_OUTPUT_URL="${ESDIAG_OUTPUT_URL}" \
        ${ESDIAG_OUTPUT_APIKEY:+--env ESDIAG_OUTPUT_APIKEY="${ESDIAG_OUTPUT_APIKEY}"} \
        ${ESDIAG_OUTPUT_USERNAME:+--env ESDIAG_OUTPUT_USERNAME="${ESDIAG_OUTPUT_USERNAME}"} \
        ${ESDIAG_OUTPUT_PASSWORD:+--env ESDIAG_OUTPUT_PASSWORD="${ESDIAG_OUTPUT_PASSWORD}"} \
        esdiag:latest "${command}" ${diag_mount:-${input}} ${*}
}

validate_image
declare command="${1}"; shift
case "${command}" in
    "collect")
        echo "The collect command is not supported in this Docker container"
        exit 1
        ;;
    "host")
        echo "The host command does not work with this Docker container."
        echo "Instead, configure your output with these environment variables:"
        echo "    - ESDIAG_OUTPUT_URL"
        echo "    - ESDIAG_OUTPUT_APIKEY"
        echo "    - ESDIAG_OUTPUT_USERNAME"
        echo "    - ESDIAG_OUTPUT_PASSWORD"
        echo
        echo " The URL is required, and either the APIKEY or USERNAME and PASSWORD."
        exit 1
        ;;
    *)
        docker_run ${*}
        ;;
esac
