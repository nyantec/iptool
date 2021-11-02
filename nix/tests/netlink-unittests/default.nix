{
  system ? builtins.currentSystem
, pkgs
, ...
}:

import (pkgs.path + "/nixos/tests/make-test-python.nix") ({ lib, ...}: with lib; {
  name = "ioctl-unittests";
  nodes.machine = {
    environment.systemPackages = with pkgs; [
      iptool-tests
      jq
      (pkgs.writeScriptBin "check-link-attr" ''
        #!${pkgs.runtimeShell}
        [ $(ip -json li  | jq -r ".[] | select(.ifname == \"$1\").$2") == "$3" ]
      '')

      killall
      tcpdump
    ];
  };

  testScript = let 
    test-bin = "iptool-test";
  in ''
    start_all()

    machine.succeed("ip li add dev loop1 type dummy")
    machine.succeed("ip li add nlmon0 type nlmon")
    machine.succeed("ip li set nlmon0 up")
    machine.succeed("tcpdump -i nlmon0 -w /run/nlmsgs.pcap &")

    machine.succeed("${test-bin} linux::netlink::test")

    machine.succeed("${test-bin} --ignored linux::netlink::test::create_dummy")

    machine.succeed("sleep 1")
    machine.succeed("killall -INT tcpdump")
    machine.succeed("sleep 1")
    machine.copy_from_vm("/run/nlmsgs.pcap", "")
  '';
}) { inherit system; }
