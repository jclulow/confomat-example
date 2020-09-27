#!/bin/bash

if ! cd /var/opt/dehydrated; then
	exit 1
fi

export PATH="/opt/dehydrated/workaround:$PATH"

exec '/opt/dehydrated/lib/dehydrated-0.6.5/dehydrated' "$@"
