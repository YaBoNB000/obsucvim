"""Register only /obf, without overwriting other commands. Run locally once."""
import os
import re
import sys
import requests

COMMAND = {
    "name": "obf", "description": "使用 obsucvim 混淆 Luau 源码（仅白名单用户）",
    "type": 1,
    "options": [{"name": "file", "description": "上传 .lua / .luau 源码，最大 256 KiB",
                 "type": 11, "required": True}],
}


def main():
    token = os.environ.get("DISCORD_BOT_TOKEN", "")
    app_id = os.environ.get("DISCORD_APPLICATION_ID", "")
    guild_id = os.environ.get("DISCORD_GUILD_ID", "")
    if not token or not re.fullmatch(r"[0-9]{1,20}", app_id) or (guild_id and not re.fullmatch(r"[0-9]{1,20}", guild_id)):
        print("Set DISCORD_BOT_TOKEN and DISCORD_APPLICATION_ID; optional DISCORD_GUILD_ID must be numeric.")
        return 1
    scope = f"/guilds/{guild_id}" if guild_id else ""
    url = f"https://discord.com/api/v10/applications/{app_id}{scope}/commands"
    try:
        with requests.Session() as session:
            session.trust_env = False
            response = session.post(url, headers={"Authorization": f"Bot {token}"},
                                    json=COMMAND, timeout=(5, 20), allow_redirects=False)
            status = response.status_code
            response.close()
    except requests.RequestException:
        print("Discord request failed. Check network access; credentials were not printed.")
        return 1
    if status not in {200, 201}:
        print(f"Registration failed (HTTP {status}). Check application ID, token and guild installation.")
        return 1
    print("Registered /obf (guild scope)." if guild_id else "Registered /obf (global scope; propagation may take time).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
