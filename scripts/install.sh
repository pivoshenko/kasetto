#!/bin/sh
# kasetto installer
# https://github.com/pivoshenko/kasetto
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/pivoshenko/kasetto/main/scripts/install.sh | sh
#
# Environment variables:
#   KASETTO_VERSION     - version tag to install (default: latest release)
#   KASETTO_INSTALL_DIR - installation directory (default: ~/.local/bin)

set -eu

REPO="pivoshenko/kasetto"

main() {
    platform="$(detect_platform)"
    arch="$(detect_arch)"
    version="${KASETTO_VERSION:-$(get_latest_version)}"
    install_dir="${KASETTO_INSTALL_DIR:-${HOME}/.local/bin}"

    target="$(detect_target "${platform}" "${arch}")"
    artifact="kasetto-${target}.tar.gz"

    log "installing kasetto ${version} (${target})"

    tmpdir="$(mktemp -d)"
    trap 'rm -rf "${tmpdir}"' EXIT

    url="https://github.com/${REPO}/releases/download/${version}/${artifact}"
    checksums_url="https://github.com/${REPO}/releases/download/${version}/checksums.txt"

    log "downloading ${url}"
    download "${url}" "${tmpdir}/${artifact}"
    download "${checksums_url}" "${tmpdir}/checksums.txt"

    log "verifying checksum"
    verify_checksum "${tmpdir}/${artifact}" "${tmpdir}/checksums.txt" "${artifact}"

    log "extracting"
    tar xzf "${tmpdir}/${artifact}" -C "${tmpdir}"

    mkdir -p "${install_dir}"
    install -m 755 "${tmpdir}/kasetto" "${install_dir}/kasetto"
    install -m 755 "${tmpdir}/kst" "${install_dir}/kst"

    log "installed kasetto to ${install_dir}/kasetto"
    log "installed kst to ${install_dir}/kst"

    if ! echo ":${PATH}:" | grep -q ":${install_dir}:"; then
        warn "add ${install_dir} to your PATH:"
        hint "  export PATH=\"${install_dir}:\${PATH}\""
    fi

    log "run 'kasetto --help' to get started"
}

detect_platform() {
    case "$(uname -s)" in
        Linux*)  echo "linux" ;;
        Darwin*) echo "darwin" ;;
        *)       err "unsupported platform: $(uname -s)" ;;
    esac
}

detect_arch() {
    case "$(uname -m)" in
        x86_64 | amd64)  echo "x86_64" ;;
        aarch64 | arm64) echo "aarch64" ;;
        *)               err "unsupported architecture: $(uname -m)" ;;
    esac
}

detect_target() {
    platform="$1"
    arch="$2"
    case "${platform}" in
        darwin) echo "${arch}-apple-darwin" ;;
        linux)  echo "${arch}-unknown-linux-gnu" ;;
        *)      err "unsupported platform: ${platform}" ;;
    esac
}

get_latest_version() {
    tag="$(download_stdout "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | head -1 \
        | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"

    if [ -z "${tag}" ]; then
        err "could not determine latest version; set KASETTO_VERSION explicitly"
    fi

    echo "${tag}"
}

download() {
    url="$1"
    dest="$2"
    if command -v curl > /dev/null 2>&1; then
        curl -fsSL "${url}" -o "${dest}"
    elif command -v wget > /dev/null 2>&1; then
        wget -qO "${dest}" "${url}"
    else
        err "neither curl nor wget found; install one and retry"
    fi
}

download_stdout() {
    url="$1"
    if command -v curl > /dev/null 2>&1; then
        curl -fsSL "${url}"
    elif command -v wget > /dev/null 2>&1; then
        wget -qO- "${url}"
    else
        err "neither curl nor wget found; install one and retry"
    fi
}

verify_checksum() {
    file="$1"
    checksums_file="$2"
    artifact="$3"

    expected="$(grep "${artifact}" "${checksums_file}" | awk '{print $1}')"
    if [ -z "${expected}" ]; then
        warn "checksum not found for ${artifact}; skipping verification"
        return
    fi

    if command -v sha256sum > /dev/null 2>&1; then
        actual="$(sha256sum "${file}" | awk '{print $1}')"
    elif command -v shasum > /dev/null 2>&1; then
        actual="$(shasum -a 256 "${file}" | awk '{print $1}')"
    else
        warn "sha256sum not found; skipping checksum verification"
        return
    fi

    if [ "${actual}" != "${expected}" ]; then
        err "checksum mismatch: expected ${expected}, got ${actual}"
    fi
}

log() {
    printf '\033[1;32m%s\033[0m %s\n' "info" "$1" >&2
}

warn() {
    printf '\033[1;33m%s\033[0m %s\n' "warn" "$1" >&2
}

hint() {
    printf '     %s\n' "$1" >&2
}

err() {
    printf '\033[1;31m%s\033[0m %s\n' "error" "$1" >&2
    exit 1
}

main
