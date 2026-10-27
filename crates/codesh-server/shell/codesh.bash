# Source explicitly from your interactive Bash startup file. Bash startup is not redirected.
[[ $- == *i* && ${CODESH_SHELL_INTEGRATION-} == 1 ]] || return
[[ ${_codesh_installed-} == 1 ]] && return
_codesh_installed=1
_codesh_running=0
_codesh_in_prompt=0
_codesh_debug_checked=0
_codesh_prompt=''
_codesh_user_prompt=''

_codesh_directory() {
    local LC_ALL=C encoded='' char hex i
    for (( i=0; i<${#PWD}; i++ )); do
        char=${PWD:i:1}
        case $char in
            [a-zA-Z0-9/._~-]) encoded+=$char ;;
            *) printf -v hex '%%%02X' "'$char"; encoded+=$hex ;;
        esac
    done
    printf '\e]7;file://%s\a' "$encoded"
}

_codesh_precmd() {
    local result=$?
    if (( _codesh_running )); then
        printf '\e]133;D;%d\a' "$result"
    fi
    _codesh_running=0
    _codesh_in_prompt=1
    _codesh_status=$result
    return "$result"
}

_codesh_prompt_end() {
    _codesh_directory
    [[ $PS1 != "$_codesh_prompt" ]] && _codesh_user_prompt=$PS1
    _codesh_prompt=$'\[\e]133;A\a\]'"$_codesh_user_prompt"$'\[\e]133;B\a\]'
    PS1=$_codesh_prompt
    _codesh_in_prompt=0
    return "$_codesh_status"
}

_codesh_prompt_status() {
    return "$_codesh_status"
}

# Bash 5 hides the caller's DEBUG trap while sourcing. Inspect it afterward,
# directly in PROMPT_COMMAND, without replacing a user's trap.
read -r -d '' _codesh_debug_setup <<'CODESH_DEBUG_SETUP' || :
if [[ ${_codesh_debug_checked-0} == 0 ]]; then
    _codesh_debug_checked=1
    if [[ -z $(trap -p DEBUG) ]]; then
        trap 'if [[ $_codesh_running == 0 && $_codesh_in_prompt == 0 && $BASH_COMMAND != _codesh_* && $BASH_COMMAND != "$PROMPT_COMMAND" ]]; then _codesh_running=1; printf "\e]133;C\a"; fi' DEBUG
    fi
fi
_codesh_prompt_status
CODESH_DEBUG_SETUP
if (( BASH_VERSINFO[0] > 5 || (BASH_VERSINFO[0] == 5 && BASH_VERSINFO[1] >= 1) )); then
    PROMPT_COMMAND=(_codesh_precmd "$_codesh_debug_setup" "${PROMPT_COMMAND[@]}" _codesh_prompt_end)
else
    PROMPT_COMMAND=$'_codesh_precmd\n'"$_codesh_debug_setup"$'\n'"${PROMPT_COMMAND-}"$'\n_codesh_prompt_end'
fi

_codesh_bind_keys() {
    local keymap sequence bindings
    for keymap in emacs-standard vi-insert vi-command; do
        bindings=$(bind -m "$keymap" -p; bind -m "$keymap" -s)
        for sequence in '\e[1;3A' '\e[1;3B'; do
            if [[ $bindings != *"\"$sequence\":"* ]]; then
                bind -m "$keymap" "\"$sequence\": \"\""
            fi
        done
    done
}
_codesh_bind_keys
unset -f _codesh_bind_keys
