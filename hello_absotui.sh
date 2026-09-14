#!/usr/bin/env bash
# Install Absotui and dependencies automagically.

# For test from the stable branch
# bash -c 'tmpfile=$(mktemp) && curl -LsSf https://github.com/pdwaldrop/absotui/raw/stable/hello_absotui.sh -o "$tmpfile" && bash "$tmpfile" install && rm -f "$tmpfile"'

set -eo pipefail

main() {
    do_not_run_as_root

    # URL variables for production (do not forget to ensure that repo name and branches are correct)
    url_config_file="https://github.com/pdwaldrop/absotui/raw/stable/config.example.toml"
    url_latest_release="https://api.github.com/repos/pdwaldrop/absotui/releases/latest"
    url_latest_binary="https://github.com/pdwaldrop/absotui/releases/download"
    url_cargo_install="https://github.com/pdwaldrop/absotui"
    url_absotui_desktop="https://raw.githubusercontent.com/pdwaldrop/absotui/stable/linux/absotui.desktop"
    url_absotui_icon="https://raw.githubusercontent.com/pdwaldrop/absotui/stable/linux/absotui.svg"

    # Verifies the running script itself against the latest release's published
    # checksum - guards against a corrupted/tampered download. `$0` is whatever this
    # script is actually running as: the tmpfile path when invoked via the documented
    # curl-pipe one-liner, or a local file path when run directly from a clone.
    #
    # This used to reference $tmpfile/$expected_sha256 as if some outer wrapper had
    # set them - it never did (the documented one-liner's `tmpfile=$(mktemp)` is a
    # plain, unexported shell variable in the *wrapper's* shell, invisible to this
    # script's own process once it's `bash "$tmpfile" install`'d as a child). Both
    # were always empty, and since they're unquoted, bash elides them from the
    # argument list entirely rather than passing empty strings - shifting every
    # later argument left by one. That made check_shasum hash (and, on the resulting
    # mismatch, delete) whatever relative path landed in the wrong slot -
    # "hello_absotui.sh" - which doesn't exist in most working directories (crashing
    # the installer immediately for virtually everyone, before install/update/
    # uninstall logic ever ran) but does, when run from inside a checked-out clone,
    # happen to be the script's own source file.
    check_shasum "$0" "hello_absotui.sh" "$(fetch_expected_checksum hello_absotui.sh)" "self"

    # Grab essential variables
    OS=$(identify_os)
    USER=${USER:-$(grab_username)}
    HOME=${HOME:-$(grab_home_dir)}
    CONFIG_DIR="${XDG_CONFIG_HOME:-$(grab_config_dir)}/absotui"
    INSTALL_DIR="${2:-$(grab_install_dir)}"

    # if gsed is needed on macos
    sed() {
        if [[ "$OS" == "macOS"  ]]; then
            command gsed "$@"
        else
            command sed "$@"
        fi
    }

    load_dependencies
    load_exit_codes

    # Adjust script to OS
    case $OS in
        linux) DISTRO="$(get_distro)";;
        macOS) DISTRO="hungry for apples?";;
        *)     install_from_source;;
    esac

    case $1 in
        --install|install) install_absotui && exit $EXIT_OK || exit $EXIT_FAIL;;
        --update|update) update_absotui && exit $EXIT_OK || exit $EXIT_FAIL;;
        --uninstall|uninstall) uninstall_absotui && exit $EXIT_OK || exit $EXIT_FAIL;;
        *) usage "INCORRECT_ARG";;
    esac
}

# `shasum` (Perl's Digest::SHA) isn't installed by default on every distro - e.g.
# Fedora's minimal/KDE spins don't pull it in, while Debian/Ubuntu-based distros
# (and macOS) do ship it, so it went unnoticed until a Fedora KDE VM install hit
# "shasum: command not found". `sha256sum` (coreutils) is the more universal choice
# on Linux, but macOS doesn't ship that - so try both, plus an `openssl` fallback
# for anything with neither.
sha256_of() {
    local target=$1
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$target" | awk "{print \$1}"
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$target" | awk "{print \$1}"
    elif command -v openssl >/dev/null 2>&1; then
        openssl dgst -sha256 "$target" | awk "{print \$NF}"
    else
        echo "[ERROR] None of sha256sum, shasum, or openssl found - cannot verify checksums." >&2
        exit 1
    fi
}

# `mktemp -d`'s failure was never checked at any of its call sites - a full
# `/tmp` (or any other mktemp failure: permissions, no TMPDIR, etc.) made it
# return nothing, silently turning every "$tmpdir/foo" path below it into
# "/foo" for the rest of that function. That produced a wall of confusing,
# unrelated-looking errors (permission denied writing to "/", "cannot stat"
# on a root-level path, a misleading "could not determine shasum from
# GitHub" for what was actually a local disk-space problem) instead of one
# clear message pointing at the real cause - confirmed live on a Linux Mint
# VM that had genuinely run out of disk space.
make_tmp_dir() {
    local dir
    dir=$(mktemp -d)
    if [[ -z "$dir" || ! -d "$dir" ]]; then
        echo "[ERROR] Could not create a temporary directory (mktemp -d failed)." >&2
        echo "This usually means \$TMPDIR/\$HOME/tmp is out of disk space, or isn't writable - check with \"df -h /tmp\" and try again." >&2
        exit 1
    fi
    echo "$dir"
}

check_shasum() {
    local tmpfile=$1
    local file_name=$2
    local expected_sha256=$3
    local file_type=$4

    if [[ -z "$expected_sha256" ]]; then
        echo "[ERROR] Could not determine the expected shasum for \"$file_name\" from GitHub."
        echo "This is not a checksum mismatch - the downloaded file was never actually compared."
        echo "It usually means the request to GitHub's API for the latest release failed (rate limit or transient network issue - wait a few minutes and try again), or that a local temp file couldn't be created (check \"df -h /tmp\" for free disk space)."
        if [[ "$file_type" == "dir" ]]; then
            rm -rf "$tmpdir"
        fi
        exit 1
    fi

    actual_sha256=$(sha256_of "$tmpfile")

    if [[ "$actual_sha256" != "$expected_sha256" ]]; then
        echo "[ERROR] Incorrect shasum for \"$file_name\""
        echo "expected shasum: "$expected_sha256""
        echo "actual shasum: "$actual_sha256""
        # Only a downloaded-into-a-tmpdir mismatch ("dir", used by every caller
        # except the self-check) gets cleaned up here - there's nothing else that's
        # ours to delete: the self-check's $tmpfile is $0, the running script,
        # deleting which would help nobody (it's either a real user's own file, or
        # a tmpfile bash is already partway through executing).
        if [[ "$file_type" == "dir" ]]; then
            rm -rf "$tmpdir"
        fi
        exit 1
    else
        echo "[INFO] shasum for "$file_name": passed"
    fi

}

# Looks up the expected checksum for a release file from SHA256SUMS.txt, a manifest
# CI generates from the release's actual uploaded assets (see release.yml). Fetched
# once per run and cached in $checksums_file - this avoids hardcoding checksums
# directly in this script, which would immediately go stale on every release since
# CI only builds the real binaries after a release is cut.
fetch_expected_checksum() {
    local file_name=$1

    if [[ -z "$full_version" ]]; then
        # `grep -o` isolates just the "tag_name": "..." fragment before sed extracts
        # the value, rather than handing sed's greedy `.*` the entire matched line -
        # GitHub doesn't guarantee pretty-printed (one-field-per-line) JSON, and on a
        # compact single-line response the old `grep tag_name | sed ...` matched the
        # *whole response* as "the line", so the greedy pattern captured whatever
        # quoted-string-then-comma happened to come last in the entire document
        # (observed live: it grabbed a chunk of the release's own body/reactions
        # data instead of the tag name).
        full_version=$(curl -s "$url_latest_release" | grep -o '"tag_name": *"[^"]*"' | head -1 | sed -E 's/.*"([^"]*)"$/\1/')
        if [[ -z "$full_version" ]]; then
            echo "[ERROR] Couldn't reach GitHub's API to determine the latest release version." >&2
            echo "This usually means a rate limit or transient network issue - wait a few minutes and try again." >&2
            return 1
        fi
    fi

    if [[ -z "$checksums_file" || ! -f "$checksums_file" ]]; then
        # A failed `mktemp` here used to be indistinguishable from a GitHub API
        # failure below it - both just left $checksums_file empty/unreadable,
        # producing the same generic "could not determine checksum from GitHub"
        # message regardless of which one actually happened. Checked explicitly
        # now, same reasoning as make_tmp_dir's own hardening above.
        checksums_file=$(mktemp)
        if [[ -z "$checksums_file" || ! -f "$checksums_file" ]]; then
            echo "[ERROR] Could not create a temporary file (mktemp failed) to hold the checksum manifest." >&2
            echo "This usually means \$TMPDIR/\$HOME/tmp is out of disk space, or isn't writable - check with \"df -h /tmp\"." >&2
            return 1
        fi

        if ! curl -LsSf "$url_latest_binary/$full_version/SHA256SUMS.txt" -o "$checksums_file" 2>/dev/null; then
            echo "[ERROR] Couldn't download the checksum manifest (SHA256SUMS.txt) for $full_version from GitHub." >&2
            echo "This usually means a rate limit or transient network issue - wait a few minutes and try again." >&2
            return 1
        fi
    fi

    grep " $file_name\$" "$checksums_file" 2>/dev/null | awk '{print $1}'
}


load_dependencies() {
    # Hard Coded dependencies here.
    # os:package_to_install(:cmd)?
    HC_DEPS=(
        arch:gnu-netcat:netcat \
        #centos:libsqlite3-dev:no_check \
        centos:nc \
        *centos:epel-release \
        debian:netcat \
        #debian:libsqlite3-dev:no_check \
        #debian:libssl-dev:no_check \
        fedora:nc \
        linux:curl \
        *linux:kitty \
        #linux:pkg-config \
        #linux:sqlite3 \
        linux:vlc  \
        macOS:curl \
        *macOS:kitty \
        macOS:netcat \
        macOS:gsed \
        #macOS:pkg-config \
        #macOS:sqlite3 \
        macOS:vlc \
        opensuse:netcat \
    )
    # Format: <OS|distrubtion>:<package_name>[:<cmd>|no_check]
    #
    # Dependencies starting with '*' are optional
    # Dependencies starting with '%' are forced
    #
    # 'linux:'  = for all linux distros
    # 'macOS:'  = macOS specific
    # 'debian:' = debian distribution specific
    # See also 'arch:', 'fedora:', 'opensuse:', 'centos:'
    #
    # (optional) ':<cmd>'
    # (optional) ':no_check'
    # INFO: Use either ':<cmd>' or ':no_check'
    #
    # By default, this script uses the package name
    # as the program name to check if the package is
    # installed. This is not sound for all packages.
    # For example: verifying "libsqlite3-dev" is
    # installed by launching "libsqlite3-dev" is
    # meaningless.
    #
    # ':<cmd>' = use <cmd> command to check for the
    # package installation on the system.
    #
    # ':no_check' = do not check whether package is
    # installed. Avoids being warned about a missing
    # dependency.
    }

identify_os() {
    case $OSTYPE in
        darwin*) os="macOS";;
        linux*)  os="linux";;
        *) os="unknown";;
    esac
    echo $os
}

grab_username() {
    local user=${USER:-$(whoami 2>/dev/null)}
    user=${user:-$(id -un 2>/dev/null)}
    if [[ -z "$user" ]]; then
        echo "[ERROR] Cannot find username."
        exit 1
    fi
    echo "$user"
}

grab_home_dir() {
    local home=${HOME:-~/$USER}
    if ! [[ -d "$home" ]]; then home=${home:-/home/$USER}; fi
    if ! [[ -d "$home" ]]; then home=${home:-/Users/$USER}; fi
    if ! [[ -d "$home" ]]; then
        echo "[ERROR] Cannot find \"$USER\" home directory."
        exit 1
    fi
    echo $home
}

grab_config_dir() {
    local config="${XDG_CONFIG_HOME}"
    if [[ $OS == "macOS" && ! -d "$config" ]]; then config="${config:-$HOME/Library/Preferences}"; fi
    if [[ $OS == "macOS" && ! -d "$config" ]]; then config="${config:-$HOME/Library/Application Support}"; fi
    if ! [[ -d "$config" ]]; then config="${config:-$HOME/.config}"; fi
    if ! [[ -d "$config" ]]; then
        echo "[ERROR] Cannot find \"$USER\" config directory."
        exit $EXIT_CONFIG
    fi
    echo "${config}"
}

grab_install_dir() {
    local install_dir="${INSTALL_DIR}"
    if [[ $OS == "linux" ]]; then
        case $DISTRO in
            *) install_dir="${install_dir:-/usr/bin}" ;;
        esac
    elif [[ $OS == "macOS" ]]; then
        install_dir="${install_dir:-/usr/local/bin}"
    fi
    if ! [[ -d "$install_dir" ]]; then
        echo "[ERROR] Cannot locate install directory \"$install_dir\"."
        exit $EXIT_INSTALL_DIR
    fi
    echo "${install_dir}"
}

usage() {
    local exit_code=$1
    echo "Usage: $ /bin/bash ./$(basename $0) <install|update> [install_directory]"
    echo "Help:"
    echo " --install: install absotui and dependencies."
    echo " --update: update absotui and dependencies."
    echo " --uninstall: uninstall absotui."
    echo "Example: /bin/bash ./$(basename $0) install /usr/bin"
    eval "exit \$EXIT_${exit_code}"
}

get_distro() {
    # Prefer ID_LIKE/ID from /etc/os-release - the standardized fields distros use to
    # declare their family (e.g. Arch derivatives like CachyOS, Manjaro, EndeavourOS all
    # set ID_LIKE=arch; Ubuntu/Debian derivatives like Pop!_OS and Linux Mint set
    # ID_LIKE="ubuntu debian"). Matching on NAME/PRETTY_NAME alone (as before) missed any
    # derivative distro whose display name doesn't literally start with the parent's name.
    local ids=""
    if [[ -f /etc/os-release ]]; then
        ids=$(. /etc/os-release 2>/dev/null && echo "$ID $ID_LIKE")
    fi
    local distro="unknown"
    case " $ids " in
        *" arch "*)                distro="archlinux";;
        *" debian "*|*" ubuntu "*) distro="debian";;
        *" fedora "*|*" rhel "*)   distro="fedora";;
        *" centos "*)              distro="centos";;
        *" suse "*|*" opensuse "*) distro="opensuse";;
        *" solus "*)               distro="solus";;
    esac

    # Fall back to the older display-name matching if os-release didn't give us
    # anything usable (missing file, or a distro we don't recognize at all).
    if [[ "$distro" == "unknown" ]]; then
        local pretty=$(head -n1 /etc/os-release 2>/dev/null| sed -E "s%.*\"([^\"]*).*\"%\1%")
        if [[ -z $pretty ]]; then pretty=$(lsb_release -a 2>/dev/null | grep Description | sed "s/Description:\s*//") ;fi
        if [[ -z $pretty ]]; then pretty=$(hostnamectl | grep "Operating System" | sed "s/Operating System:\s*//"); fi
        case "$pretty" in
            Arch*) distro="archlinux";;
            Debian*|Ubuntu*) distro="debian";;
            Fedora*) distro="fedora";;
            CentOS*) distro="centos";;
            OpenSUSE*) distro="opensuse";;
            Solus*) distro="solus";;
            *) distro="unknown";;
        esac
    fi
    echo "$distro"
}

install_brew() {
    # adapted from https://brew.sh/
    bash -c "$(sudo curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
}

install_from_source() {
    echo "[ERROR] Could not identify OS/Distro."
    echo "Please follow the instructions here:"
    echo "https://github.com/pdwaldrop/absotui?tab=readme-ov-file#git"
    exit $EXIT_UNKNOWN_OS
}

propose_optional_dependencies() {
    local optionals="$@"
    if [[ $(( ${#optionals[@]} )) == 0 || "${optionals[@]}" =~ ^\ *$ ]]; then return; fi
        echo "[INFO] Absotui's experience could be improved by these optional packages:"
        for opt in "${optionals[@]}"; do
            echo -e "\t- ${opt}"
        done
        local answer=
        while :; do
            read -p "Would you like to install these packages? (y/N) : " answer
            if [[ $answer == "" || $answer =~ (n|N) ]]; then answer=no; break; fi
            if [[ $answer =~ (y|Y) ]]; then answer=yes; break; fi
        done
        case $answer in
            no)
                echo "[INFO] Ignoring optional dependencies.";;
            yes)
                echo "[INFO] Installing optional dependencies."
                install_packages "${optionals[@]}"
                echo "[OK] Optional dependencies installed."
                ;;
        esac
    }

install_rust() {
    if ! command -v rustc >/dev/null 2>&1; then
        echo "[INFO] Cannot find \"rustc\" in your \$PATH. Installing rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
        source_cargo_env
    else
        echo "[OK] \"rustc\" exists."
    fi
}

source_cargo_env() {
    if [[ $SHELL =~ \/(sh|bash|zsh|ash|pdksh) ]]; then
        if [[ -z "${CARGO_HOME}" ]]; then
            source "$HOME/.cargo/env"
        else
            source "${CARGO_HOME}/env"
        fi
    elif [[ $SHELL =~ \/fish ]]; then
        if [[ -z "${CARGO_HOME}" ]]; then
            source "$HOME/.cargo/env.fish"
        else
            source "${CARGO_HOME}/env.fish"
        fi
    elif [[ $SHELL =~ \/nushell ]]; then
        if [[ -z "${CARGO_HOME}" ]]; then
            source "$HOME/.cargo/env.nu"
        else
            source "${CARGO_HOME}/env.nu"
        fi
    else
        echo "[ERROR] Cannot source cargo environment automatically."
        echo "Open a new terminal and launch \"hello_absotui.sh\" again."
        exit $EXIT_NO_CARGO_PATH
    fi
}

check_absotui_installed() {
    # Only the binary's own location counts as "installed" - this used to also
    # treat the config directory (~/.config/absotui, XDG_CONFIG_HOME/absotui, or
    # ~/Library/Preferences/absotui on macOS) as evidence of an install, which
    # falsely reported "already installed" for anyone whose config dir survived
    # an earlier failed/partial install (confirmed live: a Solus user hit exactly
    # this from the pre-eopkg-support install attempt leaving ~/.config/absotui
    # behind with no binary ever placed) - the config directory existing says
    # nothing about whether the binary actually got installed.
    is_installed="false"
    if [[ -e "$HOME/.cargo/bin/absotui" || -e "/usr/local/bin/absotui" ]]; then
        is_installed="true"
    fi
}

confirm_force_install_update() {
    local message_type=$1
    local message

    if [[ "$message_type" == "install" ]]; then
        message="Absotui is already installed. It's recommended to perform an uninstall before. Do you still want to force the installation? (y/N) : "
    elif [[ "$message_type" == "update" ]]; then
        message="Absotui in not installed. Install absotui before perform an update. Do you want to force update (not recommended)? (y/N) :"

    fi

    local answer=
    while :; do
        read -p "$message" answer
        if [[ -z $answer || $answer =~ (n|N) ]]; then answer=no; break; fi
        if [[ $answer =~ (y|Y) ]]; then answer=yes; break; fi
    done
    case $answer in
        no)
            exit 0
            ;;
        yes)
            ;;
    esac
}


install_packages() {
    local dep="$@"
    if (( ${#dep} == 0 )); then return; fi
    # Each install command's exit status is captured (via `|| pkg_mgr_status=$?`,
    # so `set -e` doesn't abort the whole script on a single package manager
    # failure) instead of being ignored outright - this used to unconditionally
    # print "installed successfully" even when e.g. apt genuinely failed to find
    # a candidate for a package, silently leaving a real dependency (vlc,
    # confirmed live on a Linux Mint VM) missing with no indication anything
    # was wrong.
    local pkg_mgr_status=0
    case $OS in
        linux)
	    DISTRO=${DISTRO:-$(get_distro)}
    	    case "$DISTRO" in
                arch*) sudo pacman -S --noconfirm ${dep[@]} || pkg_mgr_status=$?;;
                debian*) sudo apt install -y ${dep[@]} || pkg_mgr_status=$?;;
                fedora*) sudo dnf install -y ${dep[@]} || pkg_mgr_status=$?;;
                centos*) sudo yum install -y ${dep[@]} || pkg_mgr_status=$?;;
                opensuse*) sudo zypper install -y ${dep[@]} || pkg_mgr_status=$?;;
                solus*) sudo eopkg install -y ${dep[@]} || pkg_mgr_status=$?;;
                # Linux was identified fine - only the package manager wasn't (an
                # unlisted distro, e.g. NixOS, Void, Alpine, Gentoo). Confirmed live
                # on Solus before it had its own case above: this used to call
                # install_from_source, which prints "Could not identify OS/Distro"
                # and aborts the ENTIRE install - wrong error for "known OS, unknown
                # package manager", and needlessly fatal since nothing past this
                # point actually depends on these packages being auto-installable.
                *) echo "[WARNING] Don't know this Linux distro's package manager (${dep[@]})."; pkg_mgr_status=1;;
    	    esac ;;
        macOS)
            if command -v brew >/dev/null 2>&1; then
                brew install ${dep[@]} || pkg_mgr_status=$?
            else
                install_brew
                #echo "[ERROR] Please install \"brew\"."
                #exit $EXIT_FAIL
                fi ;;
        esac
    if [[ $pkg_mgr_status -ne 0 ]]; then
        echo "[WARNING] Your package manager reported a problem installing one or more dependencies (${dep[@]}) - see the error above. Absotui itself will still be installed, but anything needing a missing dependency (e.g. VLC for playback) won't work until you install it yourself."
    else
        echo "[INFO] Packages installed successfully."
    fi
}

post_install_msg() {
    if ! [[ -f "$CONFIG_DIR/.env" ]]; then
        echo "[INFO] No secret found in .env. Do this:"
        echo "    $ mkdir -p ~/.config/absotui"
        echo "    $ echo 'ABSOTUI_SECRET_KEY=secret' > ~/.config/absotui/.env"
    fi
}

install_config() {
    mkdir -p "$CONFIG_DIR" 2>/dev/null || ( echo "[ERROR] Cannot create config directory \"${CONFIG_DIR}\""; exit $EXIT_CONFIG )

    # .env - this key only encrypts the Audiobookshelf auth token at rest in the
    # local sqlite db; the user never needs to know or type it anywhere else, so
    # generate a strong random one silently instead of blocking install on a
    # prompt for it (this used to `read -p` here, inherited unchanged from the
    # original Toutui project - it also meant a fresh terminal install could hang
    # indefinitely if ever driven non-interactively).
    local env="${CONFIG_DIR}/.env"
    if ! [[ -f "$env" && $(sed "s/ABSOTUI_SECRET_KEY=//g" "$env") != "" ]]; then
        local key
        key=$(od -An -tx1 -N32 /dev/urandom | tr -d ' \n')
        echo "ABSOTUI_SECRET_KEY=${key}" > "$env"
        echo "[INFO] Generated a secret key to encrypt your stored Audiobookshelf login at $env"
    fi

    # config.
     # create temp directory
    local tmpdir
    tmpdir=$(make_tmp_dir)
    # dl config.example.toml in temp directory
    curl -LsSf "$url_config_file" -o "$tmpdir/config.example.toml"

    check_shasum "$tmpdir/config.example.toml" "config.example.toml" "$(fetch_expected_checksum config.example.toml)" "dir"

    local example_config="$tmpdir/config.example.toml"
    if ! [[ -f "$example_config" ]]; then
        echo "[ERROR] \"config.example.toml\" not found."
        exit $EXIT_CONFIG
    else
        # If config file exists: consider this a reinstall, and be
        # careful not to remove users configuration (e.g. themes).
        local user_config="${CONFIG_DIR}/config.toml"
        if [[ -f "$user_config" ]]; then
            # If maintainer decides adding options in "config.toml", we have to
            # update user's config file accordingly without breaking things up.
            # Here is an attempt. If you know of any way to simplify this, feel
            # free to PR <|:^)
            local merged_config=

            # Grab sections from config.example.toml AND from user config (e.g. [player])
            local sections=$(grep -Eoh "^\[.+\] *$" "$example_config" "$user_config" | awk '!visited[$0]++' | sed "s/\[//;s/\]//")
            # `awk '!visited[$0]++'` removes duplicate grep outputs, but respect the order: https://superuser.com/a/1480765

            while read section; do
                local section_uppercase=$(echo $section | tr '[:lower:]' '[:upper:]') # macOS bash3.2 doesn't support ${section^^}
                local user_extracted_section=$(sed -En "/^(#### $section_uppercase|\[$section\])/{p;:loop n;/(^####|^\[[^($section)])/q;p;b loop;}" "$user_config")
                local example_extracted_section=$(sed -En "/^(#### $section_uppercase|\[$section\])/{p;:loop n;/(^####|^\[[^($section)])/q;p;b loop;}" "$example_config")
                # Now, merge user_extracted_section with example_extracted_section
                local merged_section=
                while read example_config_line; do
		    # Prefix-match example_config_line against every line of
		    # user_extracted_section using bash's own quoted-string comparison
		    # (not grep -E) - config lines can contain anything, including
		    # regex metacharacters like the brackets in an array value, and
		    # `[[ "$a" == "$b"* ]]` treats both sides as literal text (glob
		    # rules only apply to the unquoted trailing `*`), so nothing here
		    # needs escaping.
		    local line_in_user_config=false
		    while IFS= read -r candidate_line; do
			if [[ "$candidate_line" == "$example_config_line"* ]]; then
			    line_in_user_config=true
			    break
			fi
		    done <<< "$user_extracted_section"
                    if $line_in_user_config; then
			# Keep lines that match in both example and user's config
                        merged_section+="${example_config_line}"$'\n'
                    elif [[ "$example_config_line" =~ ^([^\ ]+)\ *=\ *(.*)$ ]]; then
			# If example_config_line matches "key=value":
                        local key=${BASH_REMATCH[1]}
                        local value=${BASH_REMATCH[2]}
                        if grep -E "^${key} *=" <<< "$user_extracted_section" >/dev/null; then
			    # then if key is in user's config, keep user's value
                            local user_line="$(grep -E "^${key} *=" <<< "$user_extracted_section")"
                            [[ "$user_line" =~ ^[^\ ]+\ *=\ *(.*)$ ]]
                            local user_value="${BASH_REMATCH[1]}"
                            merged_section+="${key} = ${user_value}"$'\n'
                        else
			    # add a non-existent line in user's config from config.example.toml
                            merged_section+="${example_config_line}"$'\n'
                        fi
		    else
			# Else add a potentially commented line
                        merged_section+="${example_config_line}"$'\n'
                    fi
                done <<< "$example_extracted_section"
		# For each user's config line that is not in config.example.toml,
		# add it to new config if not already added previously. Same
		# literal-text prefix comparison as above, for the same reason.
                while read user_config_line; do
		    local line_in_merged=false
		    while IFS= read -r candidate_line; do
			if [[ "$candidate_line" == "$user_config_line"* ]]; then
			    line_in_merged=true
			    break
			fi
		    done <<< "$merged_section"
                    if ! $line_in_merged; then
                        merged_section+="${user_config_line}"$'\n'
                    fi
                done <<< "$user_extracted_section"
		# Add freshly merged section to future user's config
                merged_config+="${merged_section}"$'\n'
            done <<< "$sections"
	    # Enjoy Absotui's respect for their users' config files <|:^)
            echo -e "$merged_config" > "$user_config"
        else
            cp "$example_config" "$user_config" || (echo "[ERROR] Cannot copy \"config.toml\"."; exit $EXIT_CONFIG)
            rm -rf "$tmpdir"
        fi
    fi

    rm -rf "$tmpdir"
}

dep_already_installed() {
    local pkg_name=$1
    local cmd_check=${2:-$pkg_name}
    local installed="false"
    if [[ $OS == "linux" ]]; then
        case "$DISTRO" in
            arch*)     (pacman -Qq $pkg_name >/dev/null)2>/dev/null && installed="true";;
            debian*)   (dpkg -l | awk '{print $2}' | grep "^${pkg_name}$" >/dev/null)2>/dev/null && installed="true";;
            fedora*)   (rpm -q "$pkg_name" &>/dev/null)2>/dev/null && installed="true";;
            centos*)   (yum list installed "$pkg_name" &>/dev/null)2>/dev/null && installed="true";;
            opensuse*) (zypper se --installed-only "$pkg_name" &>/dev/null)2>/dev/null && installed="true";;
            solus*)    (eopkg list-installed | grep -qw "^${pkg_name}")2>/dev/null && installed="true";;
        esac
    elif [[ $OS == "macOS" ]]; then
        (brew list | grep $pkg_name) && installed="true"
    fi
    if [[ $installed == "false" ]]; then
        if [[ $cmd_check != "no_check" && $(command -v $cmd_check 2>/dev/null) ]]; then
            installed="true"
        fi
    fi
    echo $installed
}

install_deps() {
    # Grab dependencies and optional dependencies
    # Optional deps start with "*" (e.g. *cvlc).
    local deps=()
    local optionals=()
    if [[ -f deps.txt ]]; then
        while read -r line; do
            if [[ $line == "" || $line =~ ^\# ]]; then continue; fi
            deps+=( "$line" )
        done < deps.txt
    else
        deps=("${HC_DEPS[@]}")
    fi

    # Ignore already installed deps
    # Keep track of optional deps
    local missing=()
    for dep in "${deps[@]}"; do
        if [[ $dep =~ ^\* ]]; then
            # this is an optional dependency
            deps=("${deps[@]/$dep}") # remove optional from deps
            dep="${dep:1:${#dep}}" # trim
            local optional="true"
        elif [[ $dep =~ ^% ]];then
            # this is a forced dependency
            deps=("${deps[@]/$dep}") # remove from deps
            dep="${dep:1:${#dep}}" # trim
            deps+=( "$dep" ) # add it back
            local optional="false"
        else
            local optional="false"
        fi
        # Check if package is for OS || distro
        # linux:XXX means for all distro
        # debian:XX means specific to debian/ubuntu
        if [[ "$dep" =~ ^($OS):([^:]*)(:(.*))? || "$dep" =~ ^($DISTRO):([^:]*)(:(.*))? ]]; then
            target_sys=${BASH_REMATCH[1]}
            dep=${BASH_REMATCH[2]}
            cmd=${BASH_REMATCH[4]}
            # if OS or DISTRO match, add to optional deps
            if [[ $target_sys == $OS || $target_sys == $DISTRO ]]; then
                # add only if not installed
                if [[ $optional == "true" ]]; then
                    if [[ $(dep_already_installed "$dep" "$cmd") == "false" ]]; then
                        optionals+=( $dep )
                    fi
                else
                    if [[ $(dep_already_installed "$dep" "$cmd") == "false" ]]; then
                        echo "[DEP] Missing dependency \"$dep\""
                        missing+=( $dep )
                    fi
                fi
            fi
        fi
    done
    install_packages "${missing[@]}" && echo "[INFO] Essential dependencies are installed."
    #propose_optional_dependencies "${optionals[@]}"
}

install_message() {
    echo "[INFO]"
    echo "The installation will have these effects:"
    echo "Install dependencies if needed: VLC, Netcat, Rust, for macos: Homebrew and gsed"
    echo "Add the binary in /usr/local/bin (option 1) or ~/.cargo/bin (option 2)"
    echo "For Linux:"
    echo "Add the directory "absotui" in $HOME/.config (or any other path specified in XDG_CONFIG_HOME) with inside the following files: "
    echo ".env, db.sqlite3, config.toml, absotui.log"
    echo "absotui.desktop will be added in $HOME/.local/share/applications"
    echo "absotui.svg (app icon) will be added in $HOME/.local/share/icons/hicolor/scalable/apps"
    echo "For macOS:"
    echo "Add the directory "absotui" in $HOME/Library/Preferences (or any other path specified in XDG_CONFIG_HOME) with inside the following files: "
    echo ".env, db.sqlite3, config.toml, absotui.log"
    echo " "
    echo " You can run "absotui --uninstall"/yay -R absotui-bin or the official uninstall curl link to remove all these added files."
    echo 'Only dependencies will not be uninstalled (e.g. VLC, Netcat, Rust, gsed, Homebrew)'
    echo " "
}

install_menu() {
    echo "[HELP] Option 1 is the most user-friendly installation. No compilation time, no need to install rust/cargo. However, if it does not work, select option 2."
    ps3="Please enter your choice: "
    options=(
        "Option 1 - Download the binary (recommended)"
        "Option 2 - Compile from source (remotely, no local clone, will install Rust if it is not already installed)"
        "Option 3 - Clone the repo and compile from source locally (manually)"
        "Quit"
    )

    select opt in "${options[@]}"
    do
        case $REPLY in
            1)
                install_method="binary"
                break
                ;;
            2)
                install_method="source"
                break
                ;;
            3)
                echo "requirements:"
                echo "rust, netcat, vlc, (optional : kitty)"
                echo "follow these steps: "
                echo "clone the main branch (might be unstable):"
                echo "git clone https://github.com/pdwaldrop/absotui"
                echo "or clone the last stable release:"
                echo "git clone --branch stable --single-branch https://github.com/pdwaldrop/absotui"
                echo "cd absotui/"
                echo "mkdir -p ~/.config/absotui"
                echo "cp config.example.toml ~/.config/absotui/config.toml"
                echo "token encryption in the database (note: replace secret) : "
                echo "echo absotui_secret_key=secret >> ~/.config/absotui/.env"
                echo "cargo run --release"
                echo "update :"
                echo "git pull {URL}"
                echo "cargo run --release"
                exit 0
                break
                ;;
            4)
                echo "bye!"
                exit 0
                break
                ;;
            *)
                echo "invalid option: $REPLY"
                ;;
        esac
    done
}

check_and_cleanup_binary_install() {

    local temp_dir=$1

    if [[ ! -e "$temp_dir/absotui" ]]; then
        echo "[ERROR] Failed to download the binary. Please try again later."
        EXIT_FAIL
    fi
    if [[ -e "/usr/local/bin/absotui" && -e "$temp_dir/absotui" ]]; then
        sudo rm "/usr/local/bin/absotui"
    fi
    if [[ -e "$HOME/.cargo/bin/absotui" && -e "$temp_dir/absotui" ]]; then
        # ~/.cargo/bin is always the user's own, never root-owned (cargo install
        # itself never uses sudo) - this never needed root.
        rm "$HOME/.cargo/bin/absotui"
    fi
}

dl_handle_compressed_binary() {
    local final_url=$1
    local binary_name=$2
    local tmpdir
    tmpdir=$(make_tmp_dir)
    echo "[INFO] Downloading the compressed binary from $final_url"
    # Downloading/extracting into our own tmpdir (created by mktemp as this user)
    # never needed root either - only the final copy to /usr/local/bin does.
    curl -LsS "$final_url" -o "$tmpdir/$binary_name"

    check_shasum "$tmpdir/$binary_name" "$binary_name" "$(fetch_expected_checksum "$binary_name")" "dir"

    tar -xvzf "$tmpdir/$binary_name" -C "$tmpdir"
    check_and_cleanup_binary_install "$tmpdir"
    echo "[INFO] Copying the binary from temp directory to /usr/local/bin"
    # /usr/local/bin isn't guaranteed to already exist (confirmed live: a fresh
    # Solus VM didn't have it) - cp then fails trying to create a *file* named
    # "/usr/local/bin" instead of copying into a directory by that name.
    sudo mkdir -p "/usr/local/bin"
    sudo cp "$tmpdir/absotui" "/usr/local/bin"
    rm -rf "$tmpdir"
}

# A .desktop launcher's Exec= runs outside any login shell, so it never sees
# PATH entries added by .bashrc/.profile - only whatever the desktop session
# itself starts with. Confirmed live on Solus: the session PATH there lacks
# /usr/local/bin, so a bare `Exec=absotui` fails with "Requested executable
# not found" from the menu even though `absotui` works fine typed into a
# terminal. Resolve the actual install location once and bake that absolute
# path into Exec= instead of trusting PATH.
resolve_absotui_bin() {
    if [[ -e "/usr/local/bin/absotui" ]]; then
        absotui_bin="/usr/local/bin/absotui"
    elif [[ -e "$HOME/.cargo/bin/absotui" ]]; then
        absotui_bin="$HOME/.cargo/bin/absotui"
    else
        absotui_bin="absotui"
    fi
}

# Absotui is a TUI: it has no window of its own, so on Wayland the taskbar/dock
# icon for the *running* window is whatever terminal hosts it (the .desktop
# Icon= line only controls the launcher/pinned-icon, which is why that part
# already looked right). A handful of terminals let you override their window
# class/app_id on the command line though - if the terminal running this
# installer (detected via $TERM_PROGRAM/$TERM, which the terminal itself sets
# and which this script inherits) is one of them, build an Exec= line that
# launches absotui under its own class instead of the terminal's default one,
# paired with a matching StartupWMClass so the compositor resolves absotui's
# icon for the live window too. Leaves absotui_term_exec empty for any other
# terminal (or a non-interactive/SSH install), so the caller falls back to the
# generic Terminal=true entry - exactly what shipped before this existed.
detect_terminal_launcher() {
    absotui_term_exec=""
    absotui_term_wm_class="com.absotui.Absotui"

    local term_program_lower
    term_program_lower=$(echo "${TERM_PROGRAM:-}" | tr '[:upper:]' '[:lower:]')

    case "$term_program_lower" in
        ghostty) absotui_term_exec="ghostty --class=$absotui_term_wm_class -e $absotui_bin"; return;;
        wezterm) absotui_term_exec="wezterm start --class=$absotui_term_wm_class -- $absotui_bin"; return;;
    esac

    case "$TERM" in
        xterm-kitty) absotui_term_exec="kitty --class=$absotui_term_wm_class $absotui_bin";;
        alacritty)   absotui_term_exec="alacritty --class=$absotui_term_wm_class -e $absotui_bin";;
        foot)        absotui_term_exec="foot --app-id=$absotui_term_wm_class $absotui_bin";;
    esac
}

setup_launcher() {
    if [[ "$OS" == "linux" ]]; then
        local tmpdir
        tmpdir=$(make_tmp_dir)

        resolve_absotui_bin
        detect_terminal_launcher
        if [[ -n "$absotui_term_exec" ]]; then
            echo "[INFO] Detected a supported terminal (${TERM_PROGRAM:-$TERM}) - giving absotui its own window class ($absotui_term_wm_class) so it gets its own icon instead of the terminal's."
            cat > "$tmpdir/absotui.desktop" <<EOF
[Desktop Entry]
Name=Absotui
GenericName=Audiobookshelf client
Exec=$absotui_term_exec
Icon=absotui
Type=Application
Categories=Utility;
Terminal=false
StartupWMClass=$absotui_term_wm_class
EOF
        else
            curl -sSL "$url_absotui_desktop" -o "$tmpdir/absotui.desktop"
            check_shasum "$tmpdir/absotui.desktop" "absotui.desktop" "$(fetch_expected_checksum absotui.desktop)" "dir"
            # Checksum verifies the download is untampered; only after that do we
            # patch Exec= to the resolved absolute path (PATH isn't reliable here -
            # see resolve_absotui_bin above).
            sed -i "s|^Exec=absotui\$|Exec=$absotui_bin|" "$tmpdir/absotui.desktop"
        fi
        # ~/.local/share/... is the user's own - sudo here would plant a
        # root-owned file inside their home directory for no reason (the mkdir
        # right above is already unprivileged, so the copy should match).
        mkdir -p "$HOME/.local/share/applications"
        cp "$tmpdir/absotui.desktop" "$HOME/.local/share/applications/absotui.desktop"
        if command -v update-desktop-database >/dev/null 2>&1; then
            update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
        fi

        curl -sSL "$url_absotui_icon" -o "$tmpdir/absotui.svg"
        check_shasum "$tmpdir/absotui.svg" "absotui.svg" "$(fetch_expected_checksum absotui.svg)" "dir"
        mkdir -p "$HOME/.local/share/icons/hicolor/scalable/apps"
        cp "$tmpdir/absotui.svg" "$HOME/.local/share/icons/hicolor/scalable/apps/absotui.svg"
        if command -v gtk-update-icon-cache >/dev/null 2>&1; then
            gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
        fi

        rm -rf $tmpdir
    fi
   # elif [[ "$OS" == "macOS" ]]; then
   #     mkdir -p "/Applications/absotui.app/Contents"
   #     mkdir -p "/Applications/absotui.app/Contents/MacOS"
   #     curl -L "https://raw.githubusercontent.com/pdwaldrop/absotui/install_with_cargo/curl/Info.plist" -o "/Applications/absotui.app/Contents/Info.plist"
   #     curl -L "https://raw.githubusercontent.com/pdwaldrop/absotui/install_with_cargo/curl/launch.command" -o "/Applications/absotui.app/Contents/MacOS/launch.command"
   #     chmod +x "/Applications/absotui.app/Contents/MacOS/launch.command"
   # fi
}

install_binary() {
    # get the architecture
    arch=$(uname -m)

    # get full and latest version on github(e.g: v0.1.0-beta) - reuse it if
    # fetch_expected_checksum already resolved it earlier in this run. See
    # fetch_expected_checksum's comment above for why this needs `grep -o` +
    # `head -1` rather than a plain `grep tag_name` - GitHub doesn't guarantee
    # pretty-printed JSON, and this exact call site has been observed live
    # capturing the entire response as "the value" on a compact response.
    full_version=${full_version:-$(curl -s "$url_latest_release" | grep -o '"tag_name": *"[^"]*"' | head -1 | sed -E 's/.*"([^"]*)"$/\1/')}


    # determine binary to download
    if [[ "$OS" == "linux" && "$arch" == "x86_64" ]]; then
        echo "[INFO] Linux x86_64 detected"
        binary_name="absotui-x86_64-unknown-linux-gnu.tar.gz"
        final_url="$url_latest_binary/$full_version/$binary_name"
        dl_handle_compressed_binary "$final_url" "$binary_name"
    fi
    if [[ "$OS" == "linux" && "$arch" == "aarch64" ]]; then
        echo "[INFO] Linux aarch64 detected"
        binary_name="absotui-aarch64-unknown-linux-gnu.tar.gz"
        final_url="$url_latest_binary/$full_version/$binary_name"
        dl_handle_compressed_binary "$final_url" "$binary_name"
    fi
    if [[ "$OS" == "linux" && "$arch" != "x86_64" && "$arch" != "aarch64" ]]; then
        echo "[ERROR] No binary available for Linux $arch. You might compile from source"
        exit 0
    fi
    if [[ "$OS" == "macOS" && "$arch" == "arm64" ]]; then
        echo "[INFO] macOS arm64 detected"
        binary_name="absotui-universal-apple-darwin.tar.gz" # for intel and sillicon
        final_url="$url_latest_binary/$full_version/$binary_name"
        dl_handle_compressed_binary "$final_url" "$binary_name"
    fi
    if [[ "$OS" == "macOS" && "$arch" == "x86_64" ]]; then
        echo "[INFO] macOS x86_64 detected"
        binary_name="absotui-universal-apple-darwin.tar.gz" # for intel and sillicon
        final_url="$url_latest_binary/$full_version/$binary_name"
        dl_handle_compressed_binary "$final_url" "$binary_name"
    fi
    if [[ "$OS" == "macOS" && "$arch" != "x86_64" && "$arch" != "arm64" ]]; then
        echo "[ERROR] No binary available for macOS $arch. You might compile from source"
        exit 0
    fi
    if [[ "$OS" == "unknown" ]]; then
        echo "unknown os"
        exit 0
        break
    fi

}

confirm_install_deps_macos() {
    local answer=

    if [[ "$OS" == "macOS" ]]; then

        echo "[IMPORTANT] If you select 1, the script will automatically fetch and install the required dependencies (Brew, VLC, Netcat, gsed) if they are missing."
        echo "[IMPORTANT] Please note: package detection via Homebrew can sometimes be unreliable (but it's not risky). If you encounter issues, install the packages by yourself and select 2."

        while :; do
            read -p "Select option: (1/2/Q (to quit the installation)) : " answer
            if [[ $answer =~ (1) ]]; then answer=option1; break; fi
            if [[ $answer =~ (2) ]]; then answer=option2; break; fi
            if [[ $answer =~ (q|Q) ]]; then answer=quit; break; fi

        done
        case $answer in
            option1)
                install_deps # install essential and/or optional deps
                ;;
            option2)
                ;;
            quit)
                echo "Installation aborted. Install required dependencies Brew, VLC, Netcat and gsed and perfom again an install"
                exit 0
                ;;
        esac
    fi

}

install_absotui() {
    check_absotui_installed
    if [[ "$is_installed" == "true" ]]; then
        confirm_force_install_update "install"
    fi
    install_message
    install_menu
    if [[ "$install_method" == "binary" ]]; then
        echo "Install the binary..."
        confirm_install_deps_macos
        if [[ "$OS" == "linux" ]]; then
            install_deps # install essential and/or optional deps
        fi
        install_config # create ~/.config/absotui/ etc.
        install_binary
        setup_launcher
        if [[ "$OS" == "linux" ]]; then
            echo "[DONE] Install complete. Launch absotui from your favorite app launcher or type absotui in your terminal to run it!"
        elif [[ "$OS" == "macOS" ]]; then
            echo "[DONE] Install complete. Type absotui in your terminal to run it!"
        fi
        echo "[ADVICE] Best experience with Kitty, Ghostty, or WezTerm."
    elif [[ "$install_method" == "source" ]]; then
        echo "Compiling from source..."
        confirm_install_deps_macos
        if [[ "$OS" == "linux" ]]; then
            install_deps # install essential and/or optional deps
        fi
        install_config # create ~/.config/absotui/ etc.
        install_rust # cornerstone! absotui is written by a crab
        #cargo install --git https://github.com/pdwaldrop/absotui --branch install_with_cargo
        cargo install --git "$url_cargo_install" --branch stable
        echo "[INFO] Binary placed in ~.cargo/bin"
        setup_launcher
        # copy Absotui binary to system path
        # sudo cp ./target/release/Absotui "${INSTALL_DIR}/absotui" || exit $EXIT_BUILD_FAIL
        if [[ "$OS" == "linux" ]]; then
            echo "[DONE] Install complete. Launch absotui from your favorite app launcher or type absotui in your terminal to run it!"
        elif [[ "$OS" == "macOS" ]]; then
            echo "[DONE] Install complete. Type absotui in your terminal to run it!"
        fi
        echo "[ADVICE] Best experience with Kitty, Ghostty, or WezTerm."
        post_install_msg # only if .env not found
    fi
}


update_menu() {
    echo "[HELP] Option 1 is the most user-friendly updating method. No compilation time, no need to install rust/cargo. However, if it does not work, select option 2."
    ps3="Please enter your choice: "
    options=(
        "Option 1 - Download the binary (recommended)"
        "Option 2 - Update by compiling from source (no local clone, will install Rust if it is not already installed)"
        "Option 3 - Update from the local clone (manually)"
        "Quit"
    )

    select opt in "${options[@]}"
    do
        case $REPLY in
            1)
                update_method="binary"
                break
                ;;
            2)
                update_method="source"
                break
                ;;
            3)
                echo "cd absotui"
                echo "git pull {URL}"
                echo "cargo run --release"
                exit 0
                break
                ;;
            4)
                echo "bye!"
                exit 0
                break
                ;;
            *)
                echo "invalid option: $REPLY"
                ;;
        esac
    done
}

get_absotui_local_release() {
#    if ! [[ -f Cargo.toml ]]; then
#        echo "[ERROR] Cannot find \"Cargo.toml\"."
#        exit $EXIT_NO_CARGO_TOML
#    fi
#    grep "version" Cargo.toml | head -1 | sed -E "s/^version\s*=\s*\"([^\"]*)\"\s*$/\1/"

absotui --version | cut -d' ' -f2

}

get_absotui_github_release() {
    # See fetch_expected_checksum's comment for why this needs `grep -o` +
    # `head -1` rather than a plain `grep tag_name` - GitHub doesn't guarantee
    # pretty-printed JSON, and this exact call site has been observed live
    # capturing the entire response as "the value" on a compact response.
    curl -s "$url_latest_release" | grep -o '"tag_name": *"v[^"]*"' | head -1 | sed -E 's/.*"v([^"]*)"$/\1/'
}

display_changelog() {
    # Same `grep -o` rationale as get_absotui_github_release/install_binary above -
    # the old `grep "\"body\"" | sed "s|^\s*\"body\":..."` pattern anchored to the
    # start of the matched line, which only worked because pretty-printed JSON puts
    # "body" at the start of its own line. On a compact response the whole document
    # is one line starting with "url", so the anchor never matched and the entire
    # raw response printed unchanged.
    local changelog=$(curl -s "$url_latest_release" | grep -o '"body": *"[^"]*"' | sed -E 's/^"body": *"//; s/"$//')
    echo -e "\x1b[2m### CHANGELOG ###\x1b[0m"
    echo -e "\x1b[2m$changelog\x1b[0m"
    echo -e "\x1b[2m#################\x1b[0m"
}

check_and_cleanup_source_install() {
    if [[ -e "/usr/local/bin/absotui" ]]; then
        sudo rm "/usr/local/bin/absotui"
    fi
}

pull_latest_version() {
    local version=$1
    local answer=
    while :; do
        read -p "Would you like to update to the latest version? (Y/n) : " answer
        if [[ $answer =~ (n|N) ]]; then answer=no; break; fi
        if [[ $answer == "" || $answer =~ (y|Y) ]]; then answer=yes; break; fi
    done
    case $answer in
        no)
            echo "[INFO] Ignoring latest version.";;
        yes)
            if [[ "$OS" == "macOS" ]]; then
                echo "[INFO] gsed package is needed to correctly perform the update. Please, check if you have it ('brew install gsed' to install it)."
            fi
            # echo "[INFO] Pulling latest version..."
            # git fetch && git pull
            update_menu
            if [[ "$update_method" == "binary" ]]; then
                install_binary
            elif [[ "$update_method" == "source" ]]; then
                install_rust
                cargo install --force --git "$url_cargo_install" --branch stable
                echo "[INFO] Binary placed in ~.cargo/bin"
                check_and_cleanup_source_install
            fi
            install_config
            # Refreshes absotui.desktop and the app icon too, not just the binary/config -
            # otherwise an update never picks up changes to either (this is how the custom
            # icon added in v0.5.4-beta went unnoticed by anyone updating from an older
            # version instead of doing a fresh install).
            setup_launcher
            # cargo build --release
            # sudo cp ./target/release/Absotui "${INSTALL_DIR}/absotui" || exit $EXIT_BUILD_FAIL
            echo "[OK] Latest version installed (v$version)."
            ;;
    esac
}

update_absotui() {
    check_absotui_installed
    if [[ "$is_installed" == "false" ]]; then
        confirm_force_install_update "update"
    fi
    local local_release=$(get_absotui_local_release)
    local github_release=$(get_absotui_github_release)
    echo "[INFO] Local:  $local_release"
    echo "[INFO] GitHub: $github_release"
    if [[ $local_release == $github_release ]]; then
        echo "[INFO] Up to date (version $local_release)."
        # The desktop file/icon on the stable branch can change independently of a
        # version bump (e.g. an icon-only touch-up), and setup_launcher is otherwise
        # only reached from pull_latest_version below - so an already-up-to-date
        # install would never pick up such a change. Refresh them here too.
        setup_launcher
    else
        #echo "TODO: check if is behind or ahead?"
        if [[ "$OS" == "linux" ]]; then
            install_deps # check for new deps
        fi
        display_changelog # display before pulling?
        pull_latest_version $github_release

    fi
}

uninstall_message() {
    echo "Uninstall will do this:"
    echo "Delete the binary in /usr/local/bin or ~/.cargo/bin"
    echo "For Linux:"
    echo "The directory "absotui" in $HOME/.config (or any other path specified in XDG_CONFIG_HOME) wil be deleted: "
    echo "[IMPORTANT] save your config.toml if you need it later"
    echo "[IMPORTANT] XDG_CONFIG_HOME must be the same as it was at the time Absotui was installed. (in case you change it)"
    echo "absotui.desktop will be deleted from $HOME/.local/share/applications"
    echo "absotui.svg (app icon) will be deleted from $HOME/.local/share/icons/hicolor/scalable/apps"
    echo "For macOS:"
    echo "The directory "absotui" in $HOME/Library/Preferences (or any other path specified in XDG_CONFIG_HOME) will be deleted: "
    echo "[IMPORTANT] save your config.toml if you need it later"
    echo "[IMPORTANT] XDG_CONFIG_HOME must be the same as it was at the time Absotui was installed. (in case you change it)"
    echo " "
    echo 'Only dependencies will not be uninstalled (e.g. VLC, Netcat, Rust, Homebrew, gsed)'
    echo " "
}

uninstall_process() {
    if [[ "$OS" == "linux" ]]; then

        # delete the config folder - install_config() creates this unprivileged
        # (plain mkdir -p, no sudo), so removing it never needed root either.
        if [[ -n "$XDG_CONFIG_HOME" && -e "$XDG_CONFIG_HOME/absotui" ]]; then
            rm -r "$XDG_CONFIG_HOME/absotui"
            echo "$XDG_CONFIG_HOME/absotui deleted."
        fi

        if [[ -e "$HOME/.config/absotui" ]]; then
            rm -r "$HOME/.config/absotui"
            echo "$HOME/.config/absotui deleted."
        fi

        # delete absotui.desktopp - kept sudo'd: an install from before
        # setup_launcher() stopped using `sudo cp` here may have left this
        # root-owned, and an unprivileged rm would fail against that.
        if [[ -e "$HOME/.local/share/applications/absotui.desktop" ]] ; then
            sudo rm "$HOME/.local/share/applications/absotui.desktop"
            echo "$HOME/.local/share/applications/absotui.desktop deleted."
        fi

        # delete absotui.svg - same reasoning as absotui.desktop above.
        if [[ -e "$HOME/.local/share/icons/hicolor/scalable/apps/absotui.svg" ]] ; then
            sudo rm "$HOME/.local/share/icons/hicolor/scalable/apps/absotui.svg"
            echo "$HOME/.local/share/icons/hicolor/scalable/apps/absotui.svg deleted."
            if command -v gtk-update-icon-cache >/dev/null 2>&1; then
                gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
            fi
        fi

    fi

    if [[ "$OS" == "macOS" ]]; then

        # delete the config folder - same reasoning as the Linux branch above,
        # this was always created unprivileged.
        if [[ -n "$XDG_CONFIG_HOME" && -e "$XDG_CONFIG_HOME/absotui" ]]; then
            rm -r "$XDG_CONFIG_HOME/absotui"
            echo "$XDG_CONFIG_HOME/absotui deleted."
        fi

        if [[ -e "$HOME/Library/Preferences/absotui" ]]; then
            rm -r "$HOME/Library/Preferences/absotui"
            echo "$HOME/Library/Preferences/absotui deleted."
        fi
    fi


    # delete the binary
    if [[ -e "/usr/local/bin/absotui" ]]; then
        sudo rm "/usr/local/bin/absotui"
        echo "/usr/local/bin/absotui deleted."
    fi
    if [[ -e "$HOME/.cargo/bin/absotui" ]]; then
        # ~/.cargo/bin is always the user's own - see check_and_cleanup_binary_install.
        rm "$HOME/.cargo/bin/absotui"
        echo "$HOME/.cargo/bin/absotui deleted."
    fi


}

uninstall_absotui() {
    uninstall_message
    local answer=
    while :; do
        read -p "Are you sure to uninstall absotui? (Y/n) : " answer
        if [[ $answer =~ (n|N) ]]; then answer=no; break; fi
        if [[ $answer == "" || $answer =~ (y|Y) ]]; then answer=yes; break; fi
    done
    case $answer in
        no)
            echo "[INFO] Uninstall aborted";;
        yes)
            echo "[INFO] Starting uninstall..."
            uninstall_process
            echo "[OK] Absotui has been successfully uninstalled."
            ;;
    esac

}

load_exit_codes() {
    # Exit codes for convenience?
    EXIT_OK=0
    EXIT_FAIL=1
    EXIT_ROOT=2
    EXIT_UNKNOWN_OS=3
    EXIT_INCORRECT_ARG=4
    EXIT_NO_CARGO_TOML=5
    EXIT_NO_CARGO_PATH=6
    EXIT_CONFIG=7
    EXIT_BUILD_FAIL=8
    EXIT_INSTALL_DIR=9
}

do_not_run_as_root() {
    # Must not be run as root
    if [[ $EUID == 0 ]]; then
        echo "[ERROR] Do not run this script as root."
        exit $EXIT_ROOT
    fi
}

main "$@"

# TODO:
# - clone repo from here (making this bloated bash script "portable")
# - test automatic dependencies install on more distributions
# - allow calling absotui from outside the terminal (for macOS, available for Linux)
