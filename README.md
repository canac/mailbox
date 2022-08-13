# mailbox

`mailbox` is a CLI tool that scripts and other commands can use to asynchronously deliver messages to the user. All messages are created locally by local commands and stored locally. It is most useful for use in scripts that run in the background on a schedule. For example, a daily online backup script could create messages for the status of the backup. Or a web scraper could create messages when it finds information of interest.

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

## Overrides

`mailbox` gives you full control over how you get notified for messages, even when you don't have control over the command actually adding the messages. Suppose a non-crucial cron job adds a failure message when it can't connect to the network and you don't want to get spammed with messages every time you disconnect from WiFi. You can give `mailbox` a configuration file that overrides the state of messages or even ignores them outright based on their mailbox.

First, create a `config.toml` file somewhere on your system and format it something like this:

```toml
[overrides]
'my-script/error' = 'unread'
'my-script/log' = 'read'
'my-script/update' = 'archived'
'my-script/error/network' = 'ignored'
```

Next, set the `MAILBOX_CONFIG` environment variable to the location of that script. You probably want to add it to your shell profile to make sure that it is passed to the command that adds the message. The overrides are applied on write, so if a command doesn't have `MAILBOX_CONFIG` set or doesn't let `mailbox` inherit `MAILBOX_CONFIG` from it's environment, then `mailbox` won't know about those overrides and won't be able to apply them.

```bash
echo "export MAILBOX_CONFIG=~/config.toml" >> ~/.bash_profile
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

## Mass importing messages

Messages can also be added in bulk. Simply pipe a newline separated list of JSON message entries to `mailbox import`. The message entries have two required fields, `mailbox` and `content`, and an optional field `state` that can have the value `unread`, `read`, or `archived`.

```sh
$ printf '{"mailbox":"my-script","content":"Hello, world!"}\n{"mailbox":"my-script","content":"Hello, universe!","state":"read"}' | mailbox import
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
