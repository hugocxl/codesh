[[ -o interactive && ${CODESH_SHELL_INTEGRATION-} == 1 ]] || return
(( ${+_codesh_installed} )) && return
typeset -g _codesh_installed=1 _codesh_running=0 _codesh_prompt='' _codesh_user_prompt=''

_codesh_directory() {
    emulate -L zsh
    local LC_ALL=C encoded='' char hex
    local -i i
    for (( i=1; i<=${#PWD}; i++ )); do
        char=$PWD[i]
        if [[ $char == [a-zA-Z0-9/._~-] ]]; then
            encoded+=$char
        else
            printf -v hex '%%%02X' "'$char"
            encoded+=$hex
        fi
    done
    printf '\e]7;file://%s\a' "$encoded"
}

_codesh_precmd() {
    local result=$?
    if (( _codesh_running )); then
        printf '\e]133;D;%d\a' "$result"
    fi
    _codesh_running=0
    _codesh_directory
    [[ $PS1 != $_codesh_prompt ]] && _codesh_user_prompt=$PS1
    _codesh_prompt=$'%{\e]133;A\a%}'"$_codesh_user_prompt"$'%{\e]133;B\a%}'
    PS1=$_codesh_prompt
    return "$result"
}

_codesh_preexec() {
    _codesh_running=1
    printf '\e]133;C\a'
}

_codesh_unbound_alt() { return 0 }

# Defer until user startup files and prompt frameworks have loaded.
_codesh_install() {
    local result=$?
    precmd_functions=(${precmd_functions:#_codesh_install} _codesh_precmd)
    preexec_functions+=(_codesh_preexec)
    # Consume complete unbound Alt arrows so ZLE cannot insert a CSI suffix.
    # Keep bindings installed by the user's startup files and prompt framework.
    zle -N _codesh_unbound_alt
    local keymap sequence binding
    for keymap in emacs viins vicmd; do
        for sequence in $'\e[1;3A' $'\e[1;3B'; do
            binding=$(bindkey -M "$keymap" "$sequence")
            if [[ ${binding##* } == undefined-key ]]; then
                bindkey -M "$keymap" "$sequence" _codesh_unbound_alt
            fi
        done
    done
    _codesh_precmd
    return "$result"
}
precmd_functions+=(_codesh_install)
