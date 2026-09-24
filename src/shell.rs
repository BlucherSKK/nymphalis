use std::fs;
use std::path::Path;
use std::process::exit;

const FISH: &str = "\
# nymphalis completions — auto-generated, do not edit
set -l services gelbooru.com desu.uno mangadex.org patreon.com gel gelbu gelbooru desu md mdex mangadex patreon set add-shell-completions version update

# не предлагать файлы по умолчанию
complete -c nymphalis -f

# первый аргумент — сервис или глобальная команда
complete -c nymphalis -n 'not __fish_seen_subcommand_from $services' -a 'gelbooru.com' -d 'Download images from gelbooru'
complete -c nymphalis -n 'not __fish_seen_subcommand_from $services' -a 'desu.uno'     -d 'Download manga from desu.uno'
complete -c nymphalis -n 'not __fish_seen_subcommand_from $services' -a 'mangadex.org' -d 'Download manga from mangadex'
complete -c nymphalis -n 'not __fish_seen_subcommand_from $services' -a 'patreon.com'  -d 'Download media from Patreon'
complete -c nymphalis -n 'not __fish_seen_subcommand_from $services' -a 'set'          -d 'Save a config variable'
complete -c nymphalis -n 'not __fish_seen_subcommand_from $services' -a 'version'      -d 'Show version and platform'
complete -c nymphalis -n 'not __fish_seen_subcommand_from $services' -a 'update'       -d 'Check for updates from GitHub'
complete -c nymphalis -n 'not __fish_seen_subcommand_from $services' -a 'add-shell-completions' -d 'Install shell completions'

# псевдонимы
complete -c nymphalis -n 'not __fish_seen_subcommand_from $services' -a 'gel gelbu gelbooru' -d 'gelbooru.com alias'
complete -c nymphalis -n 'not __fish_seen_subcommand_from $services' -a 'desu'               -d 'desu.uno alias'
complete -c nymphalis -n 'not __fish_seen_subcommand_from $services' -a 'md mdex mangadex'   -d 'mangadex.org alias'
complete -c nymphalis -n 'not __fish_seen_subcommand_from $services' -a 'patreon'            -d 'patreon.com alias'

# подкоманды gelbooru
complete -c nymphalis -n '__fish_seen_subcommand_from gelbooru.com gel gelbu gelbooru' -a 'download' -d 'Download images by tags'
complete -c nymphalis -n '__fish_seen_subcommand_from gelbooru.com gel gelbu gelbooru' -a 'search'   -d 'Search tags'
complete -c nymphalis -n '__fish_seen_subcommand_from gelbooru.com gel gelbu gelbooru' -a 'set'      -d 'Set credentials'

# подкоманды desu.uno
complete -c nymphalis -n '__fish_seen_subcommand_from desu.uno desu' -a 'download' -d 'Download manga or ranobe chapters'
complete -c nymphalis -n '__fish_seen_subcommand_from desu.uno desu' -a 'search'   -d 'Search manga'
complete -c nymphalis -n '__fish_seen_subcommand_from desu.uno desu' -a 'login'    -d 'Save session from browser'

# подкоманды mangadex
complete -c nymphalis -n '__fish_seen_subcommand_from mangadex.org md mdex mangadex' -a 'download' -d 'Download Russian chapters'
complete -c nymphalis -n '__fish_seen_subcommand_from mangadex.org md mdex mangadex' -a 'search'   -d 'Search manga'

# подкоманды patreon
complete -c nymphalis -n '__fish_seen_subcommand_from patreon.com patreon' -a 'download' -d 'Download creator media'
complete -c nymphalis -n '__fish_seen_subcommand_from patreon.com patreon' -a 'show'     -d 'List active subscriptions'
complete -c nymphalis -n '__fish_seen_subcommand_from patreon.com patreon' -a 'login'    -d 'Save session from browser'

# set — переменные
complete -c nymphalis -n '__fish_seen_subcommand_from set' -a 'user_id api_key jobs desu_session desu_proxy patreon_session patreon_proxy'
";

const BASH: &str = "\
# nymphalis completions — auto-generated, do not edit
_nymphalis() {
    local cur prev words
    cur=\"${COMP_WORDS[COMP_CWORD]}\"
    prev=\"${COMP_WORDS[COMP_CWORD-1]}\"
    words=(\"${COMP_WORDS[@]}\")

    local services=\"gelbooru.com desu.uno mangadex.org patreon.com gel gelbu gelbooru desu md mdex mangadex patreon set add-shell-completions version update\"

    if [[ ${COMP_CWORD} -eq 1 ]]; then
        COMPREPLY=($(compgen -W \"$services\" -- \"$cur\"))
        return
    fi

    case \"${words[1]}\" in
        gelbooru.com|gel|gelbu|gelbooru)
            COMPREPLY=($(compgen -W \"download search set\" -- \"$cur\")) ;;
        desu.uno|desu)
            COMPREPLY=($(compgen -W \"download search login\" -- \"$cur\")) ;;
        mangadex.org|md|mdex|mangadex)
            COMPREPLY=($(compgen -W \"download search\" -- \"$cur\")) ;;
        patreon.com|patreon)
            COMPREPLY=($(compgen -W \"download show login\" -- \"$cur\")) ;;
        set)
            COMPREPLY=($(compgen -W \"user_id api_key jobs desu_session desu_proxy patreon_session patreon_proxy\" -- \"$cur\")) ;;
    esac
}

complete -F _nymphalis nymphalis
";

const ZSH: &str = "\
#compdef nymphalis
# nymphalis completions — auto-generated, do not edit

local -a services
services=(
  'gelbooru.com:Download images from gelbooru'
  'desu.uno:Download manga from desu.uno'
  'mangadex.org:Download manga from mangadex'
  'patreon.com:Download media from Patreon'
  'gel:gelbooru.com alias' 'gelbu:gelbooru.com alias' 'gelbooru:gelbooru.com alias'
  'desu:desu.uno alias'
  'md:mangadex.org alias' 'mdex:mangadex.org alias' 'mangadex:mangadex.org alias'
  'patreon:patreon.com alias'
  'set:Save a config variable'
  'version:Show version and platform'
  'update:Check for updates from GitHub'
  'add-shell-completions:Install shell completions'
)

local -a gelbooru_cmds=('download:Download images by tags' 'search:Search tags' 'set:Set credentials')
local -a desu_cmds=('download:Download manga or ranobe chapters' 'search:Search manga' 'login:Save session from browser')
local -a mangadex_cmds=('download:Download Russian chapters' 'search:Search manga')
local -a patreon_cmds=('download:Download creator media' 'show:List active subscriptions' 'login:Save session from browser')
local -a set_vars=('user_id' 'api_key' 'jobs' 'desu_session' 'desu_proxy' 'patreon_session' 'patreon_proxy')

if (( CURRENT == 2 )); then
    _describe 'service' services
    return
fi

case \"${words[2]}\" in
    gelbooru.com|gel|gelbu|gelbooru) _describe 'command' gelbooru_cmds ;;
    desu.uno|desu)                   _describe 'command' desu_cmds ;;
    mangadex.org|md|mdex|mangadex)   _describe 'command' mangadex_cmds ;;
    patreon.com|patreon)             _describe 'command' patreon_cmds ;;
    set)                             _values 'variable' $set_vars ;;
esac
";

// устанавливает файлы автодополнения для fish/bash/zsh
// если задан DESTDIR — пути относятся к нему (нужно для makepkg)
pub fn install() {
    use std::env;

    let destdir = env::var("DESTDIR").unwrap_or_default();

    let targets: &[(&str, &str, &str)] = &[
        ("fish", "/usr/share/fish/vendor_completions.d/nymphalis.fish", FISH),
        ("bash", "/usr/share/bash-completion/completions/nymphalis",    BASH),
        ("zsh",  "/usr/share/zsh/site-functions/_nymphalis",            ZSH),
    ];

    let mut any_err = false;

    for (shell, rel_path, content) in targets {
        let full = format!("{destdir}{rel_path}");
        let p = Path::new(&full);
        if let Some(dir) = p.parent() {
            if !dir.exists() {
                if let Err(e) = fs::create_dir_all(dir) {
                    eprintln!("  ✗ {shell}: cannot create {}: {e}", dir.display());
                    any_err = true;
                    continue;
                }
            }
        }
        match fs::write(p, content) {
            Ok(_)  => println!("  ✓ {shell}: {full}"),
            Err(e) => { eprintln!("  ✗ {shell}: {full}: {e}"); any_err = true; }
        }
    }

    if any_err {
        exit(1);
    }
}
