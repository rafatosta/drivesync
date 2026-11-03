#!/bin/sh
# Build the local checkout and install the selected Flatpak for the current user.
set -eu

usage() {
    printf 'Uso: %s normal|devel\n' "$0"
    printf 'Compila o código local e instala o Flatpak somente para o usuário atual.\n'
}

if [ "$#" -ne 1 ]; then
    usage >&2
    exit 2
fi

case "$1" in
    normal)
        app_id=com.rtosta.DriveSync
        ;;
    devel)
        app_id=com.rtosta.DriveSync.Devel
        ;;
    -h|--help)
        usage
        exit 0
        ;;
    *)
        usage >&2
        exit 2
        ;;
esac

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
manifest="$project_dir/flatpak/$app_id.yml"
build_dir="$project_dir/_build/$1"

if ! command -v flatpak >/dev/null 2>&1; then
    printf 'Erro: Flatpak não está instalado.\n' >&2
    exit 1
fi

if [ "$1" = normal ] && [ ! -r "$project_dir/data/oauth/production.json" ]; then
    printf 'Erro: a versão normal exige data/oauth/production.json com a configuração OAuth de produção.\n' >&2
    exit 1
fi

if command -v flatpak-builder >/dev/null 2>&1; then
    set -- flatpak-builder
elif flatpak info org.flatpak.Builder >/dev/null 2>&1; then
    set -- flatpak run org.flatpak.Builder
else
    printf 'Erro: instale flatpak-builder ou o aplicativo org.flatpak.Builder antes de continuar.\n' >&2
    exit 1
fi

cd "$project_dir"
printf 'Compilando e instalando %s para o usuário atual…\n' "$app_id"
"$@" --user --install --force-clean "$build_dir" "$manifest"
printf '\nInstalação concluída. Para abrir:\nflatpak run %s\n' "$app_id"
