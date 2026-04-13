# Shell Completions

Torc provides shell completion scripts to make working with the CLI faster and more convenient.
Completions help you discover commands, avoid typos, and speed up your workflow.

## Overview

Shell completions provide:

- **Command completion** - Tab-complete `torc` subcommands and options
- **Flag completion** - Tab-complete command-line flags and their values
- **Multi-shell support** - Bash, Zsh, Fish, Elvish, and PowerShell
- **Automatic updates** - Completions are generated from the CLI structure

## Generating Completions

Use the `torc completions` command to generate completion scripts for your shell:

```bash
# See available shells
torc completions --help

# Generate for a specific shell
torc completions bash
torc completions zsh
torc completions fish
torc completions elvish
torc completions powershell
```

## Installation

### Bash

**User installation**

```bash
# Create completions directory if it doesn't exist
mkdir -p ~/.local/share/bash-completion/completions

# Generate and install completions
torc completions bash > ~/.local/share/bash-completion/completions/torc

# Source the completion file in your current shell
source ~/.local/share/bash-completion/completions/torc
```

**Verify installation:**

```bash
# Restart your shell or source the completion file
source ~/.local/share/bash-completion/completions/torc

# Test completions
torc wor<TAB>      # Should complete to "workflows"
torc workflows <TAB>  # Should show workflow subcommands
```

### Zsh

**Option 1: User installation (recommended)**

```bash
# Create completions directory in your home directory
mkdir -p ~/.zfunc

# Add to fpath in your ~/.zshrc if not already present
echo 'fpath=(~/.zfunc $fpath)' >> ~/.zshrc
echo 'autoload -Uz compinit && compinit' >> ~/.zshrc

# Generate and install completions
torc completions zsh > ~/.zfunc/_torc

# Restart shell or source ~/.zshrc
source ~/.zshrc
```

**Option 2: Using custom location**

```bash
# Generate to a custom location
mkdir -p ~/my-completions
torc completions zsh > ~/my-completions/_torc

# Add to ~/.zshrc
echo 'fpath=(~/my-completions $fpath)' >> ~/.zshrc
echo 'autoload -Uz compinit && compinit' >> ~/.zshrc

# Restart shell
exec zsh
```

**Troubleshooting Zsh completions:**

If completions aren't working, try rebuilding the completion cache:

```bash
# Remove completion cache
rm -f ~/.zcompdump

# Restart shell
exec zsh
```

### Fish

```bash
# Fish automatically loads completions from ~/.config/fish/completions/
mkdir -p ~/.config/fish/completions

# Generate and install completions
torc completions fish > ~/.config/fish/completions/torc.fish

# Fish will automatically load the completions
# Test immediately (no shell restart needed)
torc wor<TAB>
```

### Elvish

```bash
# Create completions directory
mkdir -p ~/.elvish/lib

# Generate completions
torc completions elvish > ~/.elvish/lib/torc.elv

# Add to your ~/.elvish/rc.elv
echo 'use torc' >> ~/.elvish/rc.elv

# Restart shell
```

### PowerShell

**Windows PowerShell / PowerShell Core:**

```powershell
# Create profile directory if it doesn't exist
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $PROFILE)

# Generate completions to a file
torc completions powershell > $HOME\.config\torc_completions.ps1

# Add to your PowerShell profile
Add-Content -Path $PROFILE -Value '. $HOME\.config\torc_completions.ps1'

# Reload profile
. $PROFILE
```

**Alternative: Source inline**

```powershell
# Generate and add directly to profile
torc completions powershell | Out-File -Append -FilePath $PROFILE

# Reload profile
. $PROFILE
```

## Using Completions

Once installed, use `Tab` to trigger completions:

### Command Completion

```bash
# Complete subcommands
torc <TAB>
# Shows: workflows, jobs, files, events, run, submit, tui, ...

torc work<TAB>
# Completes to: torc workflows

torc workflows <TAB>
# Shows: create, list, get, delete, submit, run, ...
```

### Flag Completion

```bash
# Complete flags
torc --<TAB>
# Shows: --url, --username, --password, --format, --log-level, --help

torc workflows list --<TAB>
# Shows available flags for the list command

# Complete flag values (where applicable)
torc workflows list --format <TAB>
# Shows: table, json
```

### Workflow ID Completion

```bash
# Some shells support dynamic completion
torc workflows get <TAB>
# May show available workflow IDs
```

## Examples

Here are some common completion patterns:

```bash
# Discover available commands
torc <TAB><TAB>

# Complete command names
torc w<TAB>          # workflows
torc wo<TAB>         # workflows
torc j<TAB>          # jobs

# Navigate subcommands
torc workflows <TAB>  # create, list, get, delete, ...
torc jobs <TAB>       # list, get, update, ...

# Complete flags
torc --u<TAB>         # --url, --username
torc --url <type-url>
torc --format <TAB>   # table, json

# Complex commands
torc create --<TAB>
# Shows all available flags for the create command
```

## Updating Completions

When you update Torc to a new version, regenerate the completion scripts to get the latest commands
and flags:

```bash
# Bash
torc completions bash > ~/.local/share/bash-completion/completions/torc
source ~/.local/share/bash-completion/completions/torc

# Zsh
torc completions zsh > ~/.zfunc/_torc
rm -f ~/.zcompdump && exec zsh

# Fish
torc completions fish > ~/.config/fish/completions/torc.fish
# Fish reloads automatically

# PowerShell
torc completions powershell > $HOME\.config\torc_completions.ps1
. $PROFILE
```

## Automation

You can automate completion installation in your dotfiles or setup scripts:

### Bash Setup Script

```bash
#!/bin/bash
# install-torc-completions.sh

COMPLETION_DIR="$HOME/.local/share/bash-completion/completions"
mkdir -p "$COMPLETION_DIR"

if command -v torc &> /dev/null; then
    torc completions bash > "$COMPLETION_DIR/torc"
    echo "Torc completions installed for Bash"
    echo "Run: source $COMPLETION_DIR/torc"
else
    echo "Error: torc command not found"
    exit 1
fi
```

### Zsh Setup Script

```bash
#!/bin/zsh
# install-torc-completions.zsh

COMPLETION_DIR="$HOME/.zfunc"
mkdir -p "$COMPLETION_DIR"

if command -v torc &> /dev/null; then
    torc completions zsh > "$COMPLETION_DIR/_torc"

    # Add fpath to .zshrc if not already present
    if ! grep -q "fpath=(.*\.zfunc" ~/.zshrc; then
        echo 'fpath=(~/.zfunc $fpath)' >> ~/.zshrc
        echo 'autoload -Uz compinit && compinit' >> ~/.zshrc
    fi

    echo "Torc completions installed for Zsh"
    echo "Run: exec zsh"
else
    echo "Error: torc command not found"
    exit 1
fi
```

### Post-Installation Check

```bash
#!/bin/bash
# verify-completions.sh

# Test if completions are working
if complete -p torc &> /dev/null; then
    echo "✓ Torc completions are installed"
else
    echo "✗ Torc completions are not installed"
    echo "Run: torc completions bash > ~/.local/share/bash-completion/completions/torc"
fi
```

## Troubleshooting

### Completions Not Working

**Problem:** Tab completion doesn't show torc commands.

**Solutions:**

1. **Verify torc is in your PATH:**
   ```bash
   which torc
   # Should show path to torc binary
   ```

2. **Check if completion file exists:**
   ```bash
   # Bash
   ls -l ~/.local/share/bash-completion/completions/torc

   # Zsh
   ls -l ~/.zfunc/_torc

   # Fish
   ls -l ~/.config/fish/completions/torc.fish
   ```

3. **Verify completion is loaded:**
   ```bash
   # Bash
   complete -p torc

   # Zsh
   which _torc
   ```

4. **Reload shell or source completion file:**
   ```bash
   # Bash
   source ~/.local/share/bash-completion/completions/torc

   # Zsh
   exec zsh

   # Fish (automatic)
   ```

### Outdated Completions

**Problem:** New commands or flags don't show in completions.

**Solution:** Regenerate the completion file after updating Torc:

```bash
# Bash
torc completions bash > ~/.local/share/bash-completion/completions/torc
source ~/.local/share/bash-completion/completions/torc

# Zsh
torc completions zsh > ~/.zfunc/_torc
rm ~/.zcompdump && exec zsh

# Fish
torc completions fish > ~/.config/fish/completions/torc.fish
```

### Permission Denied

**Problem:** Cannot write to system completion directory.

**Solution:** Use user-level completion directory or sudo:

```bash
# Use user directory (recommended)
mkdir -p ~/.local/share/bash-completion/completions
torc completions bash > ~/.local/share/bash-completion/completions/torc

# Or use sudo for system-wide
sudo torc completions bash > /etc/bash_completion.d/torc
```

### Zsh "command not found: compdef"

**Problem:** Zsh completion system not initialized.

**Solution:** Add to your `~/.zshrc`:

```bash
autoload -Uz compinit && compinit
```

### PowerShell Execution Policy

**Problem:** Cannot run completion script due to execution policy.

**Solution:** Adjust execution policy:

```powershell
# Check current policy
Get-ExecutionPolicy

# Set policy to allow local scripts
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
```

## Shell-Specific Features

### Bash

- Case-insensitive completion (if configured in `.inputrc`)
- Partial matching support
- Menu completion available

### Zsh

- Advanced completion with descriptions
- Correction suggestions
- Menu selection
- Color support for completions

### Fish

- Rich descriptions for each option
- Real-time syntax highlighting
- Automatic paging for long completion lists
- Fuzzy matching support

### PowerShell

- IntelliSense-style completions
- Parameter descriptions
- Type-aware completions

## Best Practices

1. **Keep completions updated**: Regenerate after each Torc update
2. **Use version control**: Include completion installation in dotfiles
3. **Automate installation**: Add to setup scripts for new machines
4. **Test after updates**: Verify completions work after shell or Torc updates
5. **Document in team wikis**: Help teammates set up completions

## Additional Resources

- [Bash Completion Documentation](https://github.com/scop/bash-completion)
- [Zsh Completion System](http://zsh.sourceforge.net/Doc/Release/Completion-System.html)
- [Fish Completion Tutorial](https://fishshell.com/docs/current/completions.html)
- [PowerShell Tab Completion](https://docs.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_tab_expansion)
