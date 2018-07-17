import subprocess
import sqlite3
import sys


def main():
    print("Searching for Discord localstorage databases...")
    try:
        subprocess.check_output(["rg", "--version"])
        command = "rg ~/ --files -g 'https*.discordapp.com_0.localstorage'"
    except FileNotFoundError:
        command = "find ~/ -name 'https*.discordapp.com_0.localstorage'"

    output = subprocess.Popen(
        [command], shell=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL
    )
    results = output.communicate()[0].decode().splitlines()

    if len(results) == 0:
        print("No databases found.")
        sys.exit(1)

    print("Found:")
    for i, result in enumerate(results, start=1):
        print("{} - {}".format(i, result))

    choice = input("Select a discord storage location [1]: ")
    if choice == "":
        choice = 1
    else:
        try:
            choice = int(choice)
        except ValueError:
            print("Invalid option.")
            sys.exit(1)
    target = results[choice - 1]

    conn = sqlite3.connect(target)
    cursor = conn.cursor()
    query = cursor.execute('SELECT value FROM  ItemTable WHERE key = "token"')
    token = query.fetchone()[0].decode("utf-16-le")
    print("Your discord token is:", token)


if __name__ == "__main__":
    main()
