use std::{process::Command};

mod argparse;


fn main() {
    let raw_args: Vec<String> = std::env::args().collect();
    let mut args_it = raw_args.iter();
    args_it.next();


    let args = argparse::Parsed::from_args(&mut args_it).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(1);
    });



    let mut get_cmd = Command::new("kubectl");
    get_cmd
        .args(&args.kube_args)
        .arg("get")
        .arg(&args.target)
        .arg("-o")
        .arg("custom-columns=REPLICAS:.spec.replicas")
        .arg("--no-headers");

    let output = get_cmd.output().unwrap_or_else(|e| {
        eprintln!("failed to get number of replicas: {e}");
        std::process::exit(1);
    });

    let replicas_for_deployment: i32 = String::from_utf8(output.stdout).unwrap().trim()
        .parse().unwrap();
    let target_replicas = (args.scale_op)(replicas_for_deployment);

    let mut scale_cmd = Command::new("kubectl");
    scale_cmd
        .args(&args.kube_args)
        .arg("scale")
        .arg(&args.target)
        .arg("--replicas")
        .arg(target_replicas.to_string());

    if args.dry_run {
        eprintln!("would scale from {replicas_for_deployment} to {target_replicas}");
        eprint!("{:?}", scale_cmd);
    }  else {
        todo!("non-dry run not implemented");
    }
}
