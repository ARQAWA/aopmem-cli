# RC8 Crash Recovery Report

Before apply:

- preserve evidence;
- keep old binary unchanged;
- allow fresh recovery backup when inspect says safe.

After apply starts:

- increment `apply_attempts` before core apply;
- never auto-retry apply;
- preserve retained staged binary and recovery backup;
- fail closed unless publish-only resume is proven safe.

Installer failure messages include preserved backup paths and recovery state.
