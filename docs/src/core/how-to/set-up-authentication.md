# How to Set Up Authentication

Configure your Torc credentials so that CLI commands authenticate automatically.

## Prerequisites

Your server administrator must have enabled authentication on the Torc server. If you receive
"Authentication required" errors, follow the steps below.

## Step 1: Generate a Password Hash

Run `torc-htpasswd hash` on your machine. It prompts for your password securely (nothing appears on
screen):

```bash
torc-htpasswd hash
```

```
Password for 'alice':
Confirm password for 'alice':
Hashing password (cost=12)...
alice:$2b$12$xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
Send the line above to your server administrator.
```

The hash is safe to share — it cannot be used to recover your password.

## Step 2: Send the Hash to Your Administrator

Send the output line (`alice:$2b$12$...`) to your server administrator through any channel (email,
Slack, etc.). They will add it to the server's htpasswd file.

## Step 3: Save Your Password Persistently

**Create the credentials file** using `read -s` so the password never appears on screen or in shell
history:

```bash
mkdir -p ~/.config/torc
(
  read -s -p "Enter Torc password: " _pw && echo
  printf 'export TORC_PASSWORD="%s"\n' "$_pw"
) > ~/.config/torc/credentials
chmod 600 ~/.config/torc/credentials
```

**Source the file from your shell configuration** so it loads automatically. Add this line to
`~/.bashrc` or `~/.zshrc`:

```bash
echo '[ -f ~/.config/torc/credentials ] && source ~/.config/torc/credentials' >> ~/.bashrc
source ~/.config/torc/credentials
```

## Step 4: Verify

```bash
torc workflows list
```

## Protecting Your Credentials

- The credentials file is already restricted (`chmod 600`) from step 3
- **Never pass `--password` on the command line** — it appears in shell history and process lists
- **Do not commit** `~/.config/torc/credentials` or any file containing passwords to version control

## See Also

- [Environment Variables](../reference/environment-variables.md) — All Torc environment variables
- [CLI Reference](../reference/cli.md) — Global `--username` and `--password` flags
