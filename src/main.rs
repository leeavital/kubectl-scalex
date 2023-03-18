use std::{slice::Iter, process::Command};


fn main() {
    let args : Vec<String> = std::env::args().collect();
    let mut args_it = args.iter();
    args_it.next();

    let mut kubectl_flags : Vec<String> = Vec::new();
    let mut target = String::new();
    let mut dry_run = false;
    let mut scale_op : Option<Box<dyn FnOnce(i32) -> i32>> = None;

    while let Some(arg) = args_it.next() {
        match arg.as_str() {
            "--dry-run" => {
                dry_run = true;
            },
            "deployment" => {
                let v = consume_or_error(&mut args_it, "expected name for deployment");
                target.push_str("deployment/");
                target.push_str(v.as_str());
            },
            "statefulset" => {
                let v = consume_or_error(&mut args_it, "expected name for statefulset");
                target.push_str("statefulset/");
                target.push_str(v.as_str());
            },
            "--replicas" => {
                let v = consume_or_error(&mut args_it, "expected value for replicas flag");
                let replicas: i32  = v.parse().unwrap_or_else(|_| {
                    eprintln!("expected integer value --replicas");
                    std::process::exit(1);
                });

                scale_op = Some(Box::from(move |_: i32| { replicas }));
            },
            _ => {
                if is_single_kube_flag(arg) {
                    kubectl_flags.push(arg.to_string());
                } else if is_valued_kube_flag(arg) {
                    let emsg = format!("expected value for {}", arg);
                    let v = consume_or_error(&mut args_it, &emsg);
                    kubectl_flags.push(arg.to_string());
                    kubectl_flags.push(v.to_string());
                } else if arg.starts_with("deployment/") || arg.starts_with("statefulset/") {
                    target.push_str(arg);
                } else {
                    scale_op = match parse_op(arg) {
                        None => None, 
                        Some(f) => Some(Box::from(f)),
                    }
                }
            }
        }
    }


    if target.len() == 0 {
        eprint!("missing target (deployment or statefulset)");
        std::process::exit(1);
    }

    if scale_op.is_none() {
        eprint!("scaling operation was not specified");
        std::process::exit(1);
    }

    let mut get_cmd = Command::new("kubectl");
    get_cmd
        .args(&kubectl_flags)
        .arg("get")
        .arg(&target)
        .arg("-o")
        .arg("custom-columns=REPLICAS:.spec.replicas")
        .arg("--no-headers");

    let output = get_cmd.output().unwrap_or_else(|e| {
        eprintln!("failed to get number of replicas: {e}");
        std::process::exit(1);
    });

    let replicas_for_deployment: i32 = String::from_utf8(output.stdout).unwrap().trim()
        .parse().unwrap();
    let target_replicas = scale_op.unwrap()(replicas_for_deployment);

    let mut scale_cmd = Command::new("kubectl");
    scale_cmd
        .args(kubectl_flags)
        .arg("scale")
        .arg(target)
        .arg("--replicas")
        .arg(target_replicas.to_string());

    if dry_run {
        eprintln!("would scale from {replicas_for_deployment} to {target_replicas}");
        eprint!("{:?}", scale_cmd);
    }  else {
        todo!("non-dry run not implemented");
    }

    println!("{:?}", args);
}

const KUBE_SHORT_FLAGS : [&str; 2] = [
    "-n", // namespace
    "-c", // context
];

const KUBE_LONG_FLAGS : [&str; 25] = [
    "--as",
    "--as-group",
    "--cache-dir",
    "--certificate-authority" ,
    "--client-certificate",
    "--client-key",
    "--cluster",
    "--context",
    "--disable-compression",
    "--insecure-skip-tls-verify",
    "--kubeconfig",
    "--log-flush-frequency",
    "--match-server-version",
    "--namespace",
    "--password",
    "--profile",
    "--profile-output",
    "--server",
    "--tls-server-name",
    "--token",
    "--user",
    "--username",
    "--v",
    "--vmodule",
    "--warnings-as-errors",
];

fn is_single_kube_flag(s: &str) -> bool {
    for f in KUBE_LONG_FLAGS {
        if s.starts_with(f) && s.chars().nth(f.len()) == Some('=') {
            return true;
        }
    }
    return false;
}

fn is_valued_kube_flag(s: &str) -> bool {
    return KUBE_SHORT_FLAGS.contains(&s) || KUBE_LONG_FLAGS.contains(&s);
}

fn consume_or_error(it: &mut Iter<String>, err_msg: &str) -> String 
{
    match it.next() {
        Some(v) => v.to_string(),
        None => {
            eprint!("{}", err_msg);
            std::process::exit(1);
        }
    }
}


fn parse_op(s: &str) -> Option<impl FnOnce(i32) -> i32>
{
    let mut direction = 1;
    let mut absolute_change = 0;
    let mut factor = 1.0;
    let mut unsigned = s;
    if let Some(x) = s.strip_prefix("-") {
        direction = -1;
        unsigned =  x;
    } else if let Some(x) = s.strip_prefix("+") {
        unsigned = x;
    }


    if let Some(n) = unsigned.strip_suffix("%") {
        let parsed  : f32 = n.parse().ok()?;
        factor = (100.0 + (direction as f32 * parsed)) / 100.0;
    } else {
        let parsed : i32 = unsigned.parse().ok()?;
        absolute_change = direction * parsed;
    }

    return Some( move |x| {
        return ((x as f32 * factor).floor() + absolute_change as f32) as i32;
    });
}



#[cfg(test)]
mod test {
    use crate::{parse_op, is_single_kube_flag};



    #[test]
    fn test_parse_op() {
        fn test_expr(i: i32, xform: &str, expected: Option<i32>) {
            
            let actual = parse_op(xform).map(|scale| scale(i));
            assert_eq!(actual, expected);
        }


        test_expr(10, "+100%", Some(20));
        test_expr(10, "-100%", Some(0));
        test_expr(40, "50%", Some(60));

        test_expr(10, "hello", None);
        test_expr(10, "3", Some(13));
        test_expr(10, "+5", Some(15));
        test_expr(10, "-6", Some(4));
    }

    #[test]
    fn test_flag() {
        assert!(is_single_kube_flag("--namespace=asdfs"));
        assert!(!is_single_kube_flag("--namespaceasdfs"));
    }
}