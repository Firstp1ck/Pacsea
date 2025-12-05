# git-staged-msg

create/update commit message (short and long version) for the staged files. create/update file in dev/COMMIT/ 
Keep the commits in the following format:
- Check what kind of commit it is (feature, change or fix etc.).
each type has the following pretext:
- fix:
- feat:
- change:
- perf:
- test:
- chore:
- refactor:
- docs:
- style:
- build:
- ci:
- revert:

For each point in the commit also apply this structure:
- fix:
- feat:
- change:
- perf:
- test:
- chore:
- refactor:
- docs:
- style:
- build:
- ci:
- revert:

Complete Commit structure:
<type>: <short summary>
- <type>: point description, keep short, concise and clear.
- <type>: point description, keep short, concise and clear.
...
(No additional text or explanation)