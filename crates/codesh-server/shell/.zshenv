# Restore the user's startup directory before zsh reads the remaining startup files.
if [[ ${CODESH_ZDOTDIR_SET-} == 1 ]]; then
    export ZDOTDIR=$CODESH_ORIGINAL_ZDOTDIR
else
    unset ZDOTDIR
fi
unset CODESH_ZDOTDIR_SET CODESH_ORIGINAL_ZDOTDIR
[[ -r ${ZDOTDIR-$HOME}/.zshenv ]] && source "${ZDOTDIR-$HOME}/.zshenv"
[[ -o interactive && ${CODESH_SHELL_INTEGRATION-} == 1 ]] && source "$CODESH_SHELL_INTEGRATION_DIR/codesh.zsh"
