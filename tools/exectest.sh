#!/bin/bash

### Compare the SP sequence of two log files
### ---

usage() { echo "Usage: $0 <test.log> <correct.log> -s [fceux,nestest]" 1>&2; exit 1; }

fceux_diff() {
    ### Trim "correct" file to length of test
    test_len=`grep -c ^ "${script_args[0]}"`
    sed -i "${test_len}"',$d' "${script_args[1]}"

    test_=`sed 's/^.*[ '"${TAB}"']CYC:\(.*\)$/\1/' "${script_args[0]}"`
    correct=`sed 's/^c\([0-9]*\).*$/\1/' "${script_args[1]}"`

    sdiff -l <(echo "$test_") <(echo "$correct") | cat -n | grep -v -e '($' 1>difftest.diff
}

nestest_diff() {
    ### Trim "correct" file to length of test
    test_len=`grep -c ^ "${script_args[0]}"`
    sed -i "${test_len}"',$d' "${script_args[1]}"

    test_=`sed 's/^.*[ '"${TAB}"']CYC:\([0-9A-F]*\).*$/\1/' "${script_args[0]}"`
    correct=`sed 's/^.*CYC:\([0-9A-F]*\).*$/\1/' "${script_args[1]}"`

    sdiff -l <(echo "$test_") <(echo "$correct") | cat -n | grep -v -e '($' 1>difftest.diff
}

script_args=()
while [ $OPTIND -le "$#" ]
do
    if getopts s: option
    then
        case $option
        in
            s) style="$OPTARG";;
        esac
    else
        script_args+=("${!OPTIND}")
        ((OPTIND++))
    fi
done

TAB=$'\t'

if [ "${#script_args[@]}" != "2" ]
then
    usage
    exit 1
fi

case "${style:-fceux}"
in
    fceux) fceux_diff;;
    nestest) nestest_diff;;
    *) usage;;
esac