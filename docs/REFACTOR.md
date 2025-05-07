## New API for lazarus
In this example here, both session start and ss would refer to the same command, just some aliases

- session start <program> <block> [<week>] == ss
- session show == sh
- session cancel == sc
- session add-ex <exercise> <sets> [<reps>] == sae
- session add-set <idx> <weight> <reps> [--bw] == sas
- session edit <idx> <weight> <reps> [--bw] == ses
- session swap <idx> <variation> == sse
- session note <idx> <note> == ssn
- session history <program> <block> [--date YYYY-MM-DD]
- session end == se
- status --week --month --lifetime -> Default to week btw
- exercise add <name> <muscle> [--desc TXT]
- exercise import <file.toml>
- exercise list --muscle
- exercise show <muscle_idx> || <muscle_name>
- program import <file.toml> -> This not only creates a new program, but also updates it
- program delete <file.toml> --name [<program_name>] -> Can delete by file or name
- program list
- program show <program_name> || <program_id>
- calendar [<YYYY-MM>] [--details]
- db export [file] -> exports to a simple file output
- db import <dump.toml> -> This syncs the current db with the provided dump.toml, and if no dump.toml is provided, it creates a new database for you

## General stuff
- [x] Config file: change aliases.
- Sync: something easy to sync with github.
- Data science: progression on exercises (strength).
- Show session should grab the last occurrence of an exercise, currently it shows First time if I skipped a given ex.
- Start program should look for an ID for the program name and session. see task warrior for reference. 
- On the note above, all things should probably work with structured data.
- Release with cargo install or pkg for easy setup.
- [x] Option to output data as json for structured data (nushell shit).
- [x] `ex list chest` if I want to grab all chest exercises.
- Add neck as muscle.
- [x] Global indeces for exercises/programs/sets/whatever (this way it's easier to use them in a phone)

## Notes
- Aliases: don’t mix a top-level alias with a built-in ex/s token.
- Nushell does not automatically recognizes json output from lazaro, we'd have to keep using `| from json` everywhere. In the future, create a small wrapper to add in nushell for this (when I make the release).
- Structured data is a must.
