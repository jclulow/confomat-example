#!/bin/bash

case "$1" in
deploy_cert)
	printf 'INFO: testing nginx configuration\n' >&2
	if ! /opt/local/sbin/nginx -t; then
		printf 'FATAL: nginx configuration error\n' >&2
		exit 1
	fi

	printf 'INFO: restarting nginx\n' >&2
	/usr/sbin/svcadm restart 'svc:/pkgsrc/nginx:default'
	;;
esac
