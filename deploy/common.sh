#!/bin/bash

function deploy_to {
	local host=$1

	if [[ -z "$dir" ]]; then
		printf 'ERROR: must set $dir\n' >&2
		exit 100
	fi

	if ! ssh root@$host type rsync >/dev/null; then
		printf 'INFO: installing rsync on "%s"...\n' "$host" >&2
		if ! ssh root@$host pkg install rsync; then
			exit 1
		fi
	fi

	mkdir -p "$dir/bin"
	rm -f "$dir/bin/confomat"
	if ! ln "$dir/target/release/confomat" "$dir/bin/confomat"; then
		printf 'ERROR: must build "confomat" binary\n' "$dir" >&2
		exit 1
	fi

	printf 'INFO: host %s: copying files...\n' "$host" >&2
	if ! rsync -Pa --delete --delete-excluded \
	    --exclude .git \
	    --exclude /deploy \
	    --exclude /target \
	    --exclude /src \
	    "$dir/" \
	    "root@$host:/root/confomat/"; then
		exit 1
	fi

	printf 'INFO: host %s ok\n' "$host" >&2
}
