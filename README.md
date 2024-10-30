# mailbox

`mailbox` is a message manager for output from local and remote scripts. To illustrate, when you run a backup script directly in your terminal you can see the output and status code immediately in stdout/stderr. However, if you run that script daily on a schedule via cron, you need another way to see whether it succeeded and get other feedback. You could write the output to a log file but then you need to repeatedly check it for updates and keep track of which log messages are new. That is where `mailbox` comes in. You can configure the backup script to add a mailbox message about how many files were backed or whether the command failed. You can then asynchronously review these messages at your convenience as they arrive. `mailbox` also lets you organize messages into mailboxes and keep track of which messages have been read already. You can even integrate `mailbox` with your shell prompt to be quickly notified in your terminal of any new, unread messages.

`mailbox` can be used with either a local database or a remote database.

1. **Local database**: `mailbox` can store its messages in a local SQLite database. Local commands and scripts can add messages through a CLI interface. You can then use that CLI to review and manipulate messages. This approach is the most performant when all messages are created and viewed on the same machine.
2. **Remote database**: `mailbox` can also communicate with the REST interface provided by a [`mailbox-server`](./server) running on a remote machine. `mailbox-server` stores its messages in a SQLite database on the machine it runs on. Commands and scripts can add messages through the `mailbox` CLI or directly by using the REST API documented [here](./server/README.md). You can use the `mailbox` CLI to review and manipulate messages.

## Installation

Install `mailbox` via Homebrew.

```sh
$ brew install canac/tap/mailbox
```

## Adding a message

The first step is creating a new message.

```sh
$ mailbox add my-script "Hello, world!"
* Hello, world! [my-script] @ now
```

Messages are organized into mailboxes. A mailbox is simply a collection of messages. The first argument to `mailbox add` is the mailbox name. The second argument is the message content. The output shows that a new message was created. The mailbox name is between the square brackets, and the timestamp is after the @ sign. The asterisk (\*) at the beginning is an indicator that the message hasn't been read yet.

## Reading messages

As messages are created in the background, the next step is to read them. There are a couple of options. `mailbox view` shows all unread messages.

```sh
$ mailbox view
* Hello, world! [my-script] @ now
```

`mailbox read` shows all unread messages _and_ marks them as read. It also displays all the messages that were read.

```sh
$ mailbox read
  Hello, world! [my-script] @ now

$ mailbox view

```

Notice that the unread asterisk is gone and `mailbox view` no longer shows the message because it is unread.

`mailbox archive` archives all messages, whether read or unread, and marks them as read. It also displays all the messages that were archived.

```sh
$ mailbox archive
- Hello, world! [my-script] @ now
```

The dash (-) is an indicator that the message is now archived.

Messages can be in one of three states:

- Unread (\* indicator)
- Read (no indicator)
- Archived (- indicator)

`mailbox view` only shows unread messages by default, but read and archived messages can be viewed by passing the `--state` flag.

```sh
$ mailbox view --state=archived
- Hello, world! [my-script] @ now
```

The possible values for state are:

- `unread` (only unread messages, default)
- `read` (only read messages)
- `archived` (only archived messages)
- `unarchived` (only unread and read messages)
- `all` (all messages, regardless of state)

Messages can also be filtered by their mailbox with the `--mailbox` flag. `view`, `read`, `archive`, and `clear` all accept the `--mailbox` flag. When messages are categorized into sub mailboxes separated by a `/` (`first-script/errors`, for example), the `--mailbox` filter include the parent mailbox and any sub mailboxes.

```sh
$ mailbox add first-script "Hello, world!"
* Hello, world! [first-script] @ now

$ mailbox add second-script "Hello, universe!"
* Hello, world! [second-script] @ now

$ mailbox add second-script/errors "Whoops!"
* Hello, world! [second-script/errors] @ now

$ mailbox view
* Hello, world! [first-script] @ now
* Hello, world! [second-script] @ now
* Whoops! [second-script/errors] @ now

$ mailbox view --mailbox=second-script
* Hello, world! [second-script] @ now
* Whoops! [second-script/errors] @ now

$ mailbox view --mailbox=second-script/errors
* Whoops! [second-script/errors] @ now
```

## Clearing messages

The final stage of a message's lifecycle is being deleted. `mailbox clear` permanently clears all archived messages.

```sh
$ mailbox clear
- Hello, world! [my-script] @ now

$ mailbox view --state=all

```

## Typical workflow

A typical workflow when using mailbox is to first check for any new messages by running `mailbox view`. Then, if there aren't any messages that you want to continue to be reminded about, run `mailbox read`. Alternatively, when you don't want to see any of those messages again, run `mailbox archive`. Periodically, optionally run `mailbox clear` to prevent archived messages from building up.

## TUI

You can also view the mailbox messages in an interactive terminal UI by running `mailbox tui`.

The following keyboard commands allow navigating and performing operations on the messages.

### Global commands

- `1`: focus the mailboxes pane
- `2`: focus the messages pane
- `right` or `left`: switch between mailboxes and messages panes
- `Shift+r`: refresh the messages list
- `Ctrl+u`: toggle whether unread messages are displayed
- `Ctrl+r`: toggle whether read messages are displayed
- `Ctrl+a`: toggle whether archived messages are displayed
- `q`: exit the TUI

### Mailbox pane commands

- `j` or `down`: move the cursor down one mailbox
- `k` or `up`: move the cursor up one mailbox
- `Shift+k`: move the cursor to the parent mailbox
- `Ctrl+j` or `Ctrl+down`: move the cursor to the next mailbox in the tree at the same depth or shallower, skipping over deeper mailboxes
- `Ctrl+k` or `Ctrl+up`: move the cursor to the previous mailbox in the tree at the same depth or shallower, skipping over deeper mailboxes
- `Escape`: remove the cursor
- `u`: mark all visible messages in the selected mailbox as unread
- `r`: mark all visible messages in the selected mailbox as read
- `a`: mark all visible messages in the selected mailbox as archived

### Message pane commands

- `j` or `down`: move the cursor down one message
- `k` or `up`: move the cursor up one message
- `Shift+j`: move the cursor to the first message
- `Shift+k`: move the cursor to the last message
- `Ctrl+j` or `Ctrl+down`: move the cursor down ten messages
- `Ctrl+k` or `Ctrl+up`: move the cursor up ten messages
- `Escape`: remove the cursor
- `Space`: toggle whether the message under the cursor is selected
- `g`: select all messages
- `Shift+g`: deselect all messages
- `Ctrl+s`: toggle whether moving the cursor also selects messages
- `Ctrl+d`: toggle whether moving the cursor also deselects messages
- `u`: mark the selected messages or the message under the cursor as unread
- `r`: mark the selected messages or the message under the cursor as read
- `a`: mark the selected messages or the message under the cursor as archived
- `Ctrl+x`: delete the selected messages or the message under the cursor
- `Enter`: open the URL in the message under the cursor in a web browser

### Command line arguments

You can also set the initial message filters by passing the `--state` or `--mailbox` command line arguments, similar to `mailbox view`.

```sh
# Unread and read messages are initially shown
$ mailbox tui --state=unarchived

# Read messages in the online-backup mailbox are initially shown
$ mailbox tui --state=read --mailbox=online-backup

```

## Starship notifications

You'll probably want to get notifications for your unread messages somehow. A custom terminal prompt via [Starship](https://starship.rs) is a great way to do that! Add this to `~/.config/starship.toml` enable mailbox notifications:

```toml
# Put the mailbox notifications before all other modules
format = "${custom.mailbox}$all"

[custom.mailbox]
# Count the number of unread messages, and display the count if there are any
command = 'export count=$(mailbox view | wc -l) && test $count -gt 0 && echo $count'
when = true
format = '[($output )](bold yellow)'
shell = ['bash', '--noprofile', '--norc']
```

If you are using a remote database, you might want to cache the output of `mailbox view` to keep updating the prompt quick. You can use a tool like [`bkt`](https://github.com/dimo414/bkt) to achieve that.

```toml
# Put the mailbox notifications before all other modules
format = "${custom.mailbox}$all"

[custom.mailbox]
# Count the number of unread messages, and display the count if there are any
# Keep cached results for one day and update the message count in the background every minute
command = 'export count=$(bkt --ttl=1d --stale=1m -- mailbox view | wc -l) && test $count -gt 0 && echo $count'
when = true
format = '[($output )](bold yellow)'
shell = ['bash', '--noprofile', '--norc']
```

## Colors

By default, colored output is only enabled if the terminal is a TTY. Colors can be forced on by setting the environment variable `CLICOLOR_FORCE=1` or passing the `--color` flag. Colors can be forced off by setting the environment variable `CLICOLOR=0` or `NO_COLOR=1` or passing the `--no-color` flag.

## Timestamp format

The output format of timestamps can be controlled with the `--timestamp-format` flag. The possible values are `relative` (timestamps like `15 minutes ago`), `local` (timestamps in local time), and `utc` (timestamps in UTC time).

## Overrides

`mailbox` gives you full control over how you get notified for messages, even when you don't have control over the command actually adding the messages. Suppose a non-crucial cron job adds a failure message when it can't connect to the network and you don't want to get spammed with messages every time you disconnect from WiFi. You can create a configuration file that overrides the state of messages or even ignores them outright based on their mailbox.

First, make sure that the `$EDITOR` environment variable is set to your preferred editor. Then run `mailbox config edit` to open the configuration file in your configured editor. For example:

```sh
$ EDITOR=code mailbox config edit
```

Next, type something like this and save the file:

```toml
[overrides]
'my-script/error' = 'unread'
'my-script/log' = 'read'
'my-script/update' = 'archived'
'my-script/error/network' = 'ignored'
```

Now the state of messages that my-script adds will be overridden.

```sh
# State will be overridden from read -> unread
$ mailbox add my-script/error "Something catastrophic happened!" --state=read
* Something catastrophic happened! [my-script/error] @ now

# State will not be overridden
$ mailbox add my-script/warning "Something less catastrophic happened" --state=read
  Something less catastrophic happened [my-script/error] @ now

# State will be overridden from unread -> archived
$ mailbox add my-script/update "New my-script version available!" --state=unread
- New my-script version available! [my-script/update] @ now

# Message will be ignored entirely
$ mailbox add my-script/error/network "Couldn't connect to the internet" --state=read

```

You can also run `mailbox config locate` to print the OS-dependent path of the configuration file.

## Using a remote database

By default, messages are stored in a local SQLite database. To use a remote database instead, first start [`mailbox-server`](./server/README.md) on the machine that you want to host the database. It will use a local SQLite database and expose a REST API over HTTP to interact with the mailbox.

```sh
$ mailbox-server --expose --token=0a1b2c3de4f5 # token can be any string
```

Then on the client machine, add the following to your configuration file:

```toml
[database]
provider = 'http'
url = 'http://10.0.0.10:8080' # replace with the IP address and port of the mailbox server
token = '0a1b2c3de4f5' # optional, replace with with the API token passed to `mailbox-server --token=xxx`
```

This repository contains a reference implementation of the HTTP server written in Rust. However, `mailbox` can connect to any provider over HTTP as long as it fulfills the API contract documented here [`mailbox-server`](./server/README.md#rest-api). Alternative HTTP servers can be written in other languages and even use a different other than SQLite.

## Mass importing messages

Messages can also be added in bulk. Simply pipe a newline separated list of tab separated message entries to `mailbox import`. The first field is the mailbox, the second field is the content, and the optional third field is the state and must have the value `unread`, `read`, or `archived`.

```sh
$ printf 'my-script\tHello, world!\nmy-script\tHello, universe!\tread' | mailbox import
* Hello, world! [my-script] @ now
  Hello, universe! [my-script] @ now
```

Alternatively, you can pipe in a newline separated list of JSON message entries and pass the `--format=json` flag. The message entries have two required fields, `mailbox` and `content`, and an optional field `state` that can have the value `unread`, `read`, or `archived`.

```sh
$ printf '{"mailbox":"my-script","content":"Hello, world!"}\n{"mailbox":"my-script","content":"Hello, universe!","state":"read"}' | mailbox import --format=json
* Hello, world! [my-script] @ now
  Hello, universe! [my-script] @ now
```

## Full output

By default, `mailbox` tries to make its output fit within the available terminal space. To achieve this, it truncates long messages and summarizes mailboxes containing many messages.

```sh
$ mailbox add my-script "This is a super long message that will need to be truncated"
$ mailbox add my-script/mailbox-1 "Message 1"
$ mailbox add my-script/mailbox-1 "Message 2"
$ mailbox add my-script/mailbox-1 "Message 3"
$ mailbox add my-script/mailbox-1 "Message 4"
$ mailbox add my-script/mailbox-1 "Message 5"
$ mailbox add my-script/mailbox-1 "Message 6"
$ mailbox add my-script/mailbox-1 "Message 7"
$ mailbox add my-script/mailbox-1 "Message 8"
$ mailbox add my-script/mailbox-1 "Message 9"
$ mailbox add my-script/mailbox-1 "Message 10"
$ mailbox add my-script/mailbox-1 "Message 11"
$ mailbox add my-script/mailbox-1 "Message 12"
$ mailbox add my-script/mailbox-2 "Message 1"
$ mailbox add my-script/mailbox-2 "Message 2"
$ mailbox add my-script/mailbox-2 "Message 3"
$ mailbox add my-script/mailbox-2 "Message 4"
$ mailbox add my-script/mailbox-2 "Message 5"
$ mailbox add my-script/mailbox-2 "Message 6"
$ mailbox add my-script/mailbox-2 "Message 7"
$ mailbox add my-script/mailbox-2 "Message 8"

$ mailbox view
* This is a super long message that will need … [my-script] @ now
* Message 1 [my-script/mailbox-1] @ now
* Message 2 [my-script/mailbox-1] @ now
* Message 3 [my-script/mailbox-1] @ now
* Message 4 [my-script/mailbox-1] @ now (+8 older messages)
* Message 1 [my-script/mailbox-2] @ now
* Message 2 [my-script/mailbox-2] @ now
* Message 3 [my-script/mailbox-2] @ now (+5 older messages)
```

To view the messages without summarization or truncation, pass the `--full-output`/`-f` flag.

```sh
$ mailbox view --full-output
* This is a super long message that will need to be truncated [my-script] @ now
* Message 1 [my-script/mailbox-1] @ now
* Message 2 [my-script/mailbox-1] @ now
* Message 3 [my-script/mailbox-1] @ now
* Message 4 [my-script/mailbox-1] @ now
* Message 5 [my-script/mailbox-1] @ now
* Message 6 [my-script/mailbox-1] @ now
* Message 7 [my-script/mailbox-1] @ now
* Message 8 [my-script/mailbox-1] @ now
* Message 9 [my-script/mailbox-1] @ now
* Message 10 [my-script/mailbox-1] @ now
* Message 11 [my-script/mailbox-1] @ now
* Message 12 [my-script/mailbox-1] @ now
* Message 1 [my-script/mailbox-2] @ now
* Message 2 [my-script/mailbox-2] @ now
* Message 3 [my-script/mailbox-2] @ now
* Message 4 [my-script/mailbox-2] @ now
* Message 5 [my-script/mailbox-2] @ now
* Message 6 [my-script/mailbox-2] @ now
* Message 7 [my-script/mailbox-2] @ now
* Message 8 [my-script/mailbox-2] @ now
```
