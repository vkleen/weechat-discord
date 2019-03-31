import subprocess
import sys
import string
import platform


def strings(filename, min=4):
    with open(filename, errors="ignore") as f:
        result = ""
        for c in f.read():
            if c in string.printable:
                result += c
                continue
            if len(result) >= min:
                yield result
            result = ""
        if len(result) >= min:
            yield result


def run_command(cmd):
    output = subprocess.Popen(
        [cmd], shell=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL
    )
    return output.communicate()[0].decode().splitlines()


def main():
    print("Searching for Discord localstorage databases...")
    rg = False
    if platform.system() == "Darwin":
        results = run_command("mdfind \"kMDItemDisplayName=='*.ldb'\"")
    else:
        try:
            subprocess.check_output(["rg", "--version"])
            results = run_command("rg ~/ --files -g '*.ldb'")
            rg = True
        except FileNotFoundError:
            results = run_command("find ~/ -name '*.ldb'")

    if len(results) == 0 and rg:
        results = run_command("rg ~/ --hidden  --files -g '*.ldb'")

    if len(results) == 0:
        print("No databases found.")
        sys.exit(1)

    discord_databases = list(filter(lambda x: "discord" in x, results))

    token_results = set()
    for database in discord_databases:
        for tok in strings(database):
            if len(tok) == 61:
                if tok.count(".") == 2:
                    token_results.add(tok[1:-1])

    print("Likely Discord tokens are:\n")
    for token in token_results:
        print(token)


if __name__ == "__main__":
    main()
