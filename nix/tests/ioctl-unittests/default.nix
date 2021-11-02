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
    ];
  };

  testScript = let 
    test-bin = "iptool-test";
  in ''
    start_all()

    machine.succeed("ip li add dev loop1 type dummy")

    machine.succeed("check-link-attr loop1 operstate DOWN")
    machine.succeed("${test-bin} --ignored linux::test::up")
    machine.fail("check-link-attr loop1 operstate DOWN")

    machine.fail("check-link-attr loop1 mtu 1420")
    machine.succeed("${test-bin} --ignored linux::test::mtu")
    machine.succeed("check-link-attr loop1 mtu 1420")

    machine.fail("check-link-attr loop1 operstate DOWN")
    machine.succeed("${test-bin} --ignored linux::test::down")
    machine.succeed("check-link-attr loop1 operstate DOWN")

    machine.fail("check-link-attr loop1 address 5a:e6:60:8f:5f:de")
    machine.succeed("${test-bin} --ignored linux::test::mac")
    machine.succeed("check-link-attr loop1 address 5a:e6:60:8f:5f:de")
  '';
}) { inherit system; }
