#!/bin/bash

### Compare the SP sequence of two log files
### ---

usage() { echo "Usage: $0 <test.log> <correct.log>" 1>&2; exit 1; }

if [ -z "${1}" ] || [ -z "${2}" ]
then
    usage
fi

TAB=$'\t'

### Trim "correct" file to length of test
test_len=`grep -c ^ "${1}"`
sed -i ".bak" "${test_len}"',$d' "${2}"

test_=`sed 's/^.*[ '"${TAB}"']CYC:\(.*\)$/\1/' "${1}"`
correct=`sed 's/^c\([0-9]*\).*$/\1/' "${2}"`

sdiff -l <(echo "$test_") <(echo "$correct") | cat -n | grep -v -e '($' 1>spdiff.diff