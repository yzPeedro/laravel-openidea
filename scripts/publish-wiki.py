#!/usr/bin/env python3
import os
import re
import shutil
import subprocess
import sys
import tempfile

REPO = "yzPeedro/laravel-openidea"
BLOB = f"https://github.com/{REPO}/blob/main/"
RAW = f"https://raw.githubusercontent.com/{REPO}/main/"
WIKI_GIT = f"https://github.com/{REPO}.wiki.git"

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SOURCE = os.path.join(ROOT, "wiki")


def convert(text, pages):
    def link(match):
        label, target, anchor = match.group(1), match.group(2), match.group(3) or ""
        if target.startswith("../"):
            return f"[{label}]({BLOB}{target[3:]}{anchor})"
        if target in pages:
            return f"[{label}]({target[:-3]}{anchor})"
        return match.group(0)

    text = re.sub(r"\[([^\]]*)\]\(([^)#\s]+\.md)(#[^)]*)?\)", link, text)
    text = re.sub(r'src="\.\./([^"]+)"', lambda m: f'src="{RAW}{m.group(1)}"', text)
    text = re.sub(r"\]\(\.\./([^)]+)\)", lambda m: f"]({BLOB}{m.group(1)})", text)
    return text


def sidebar(home, pages):
    lines = ["**[Laravel OpenIDEA](Home)**", ""]
    for block in re.split(r"\n## ", home)[1:]:
        title, _, body = block.partition("\n")
        links = re.findall(r"^\| \[([^\]]+)\]\(([A-Za-z0-9-]+)\.md\)", body, re.M)
        if not links:
            continue
        lines.append(f"**{title.strip()}**")
        lines.append("")
        lines.extend(f"- [{label}]({target})" for label, target in links if f"{target}.md" in pages)
        lines.append("")
    return "\n".join(lines)


def main():
    pages = {name for name in os.listdir(SOURCE) if name.endswith(".md")}
    sha = subprocess.run(["git", "-C", ROOT, "rev-parse", "--short", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
    work = tempfile.mkdtemp(prefix="laravel-openidea-wiki-")
    try:
        subprocess.run(["git", "clone", "--quiet", WIKI_GIT, work], check=True)
        for name in os.listdir(work):
            if name == ".git":
                continue
            path = os.path.join(work, name)
            shutil.rmtree(path) if os.path.isdir(path) else os.remove(path)
        for name in pages:
            with open(os.path.join(SOURCE, name), encoding="utf-8") as f:
                text = convert(f.read(), pages)
            with open(os.path.join(work, name), "w", encoding="utf-8", newline="\n") as f:
                f.write(text)
        shutil.copytree(os.path.join(SOURCE, "images"), os.path.join(work, "images"))
        with open(os.path.join(SOURCE, "Home.md"), encoding="utf-8") as f:
            side = sidebar(f.read(), pages)
        with open(os.path.join(work, "_Sidebar.md"), "w", encoding="utf-8", newline="\n") as f:
            f.write(side)
        subprocess.run(["git", "-C", work, "add", "--all"], check=True)
        if subprocess.run(["git", "-C", work, "diff", "--cached", "--quiet"]).returncode == 0:
            print("The wiki is already up to date")
            return
        message = f"Publish the wiki from wiki/ at {sha}"
        subprocess.run(["git", "-C", work, "commit", "--quiet", "-m", message], check=True)
        if "--dry-run" in sys.argv:
            print(f"Dry run: {work} holds the converted wiki, nothing was pushed")
            return
        subprocess.run(["git", "-C", work, "push", "--quiet", "origin", "HEAD"], check=True)
        print(f"Published {len(pages)} pages to https://github.com/{REPO}/wiki")
    finally:
        if "--dry-run" not in sys.argv:
            shutil.rmtree(work, ignore_errors=True)


main()
