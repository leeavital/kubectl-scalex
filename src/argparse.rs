use std::slice::Iter;

pub struct Parsed {
    pub kube_args: Vec<String>,
    pub dry_run: bool,
    pub scale_op: Box<dyn FnOnce(i32) -> i32>,
    pub target: String,
}

impl  Parsed {

    pub fn from_args(args_it: &mut Iter<String>) -> Result<Parsed, String> {

        let mut dry_run = false;
        let mut target  = String::new();
        let mut scale_op: Option<Box<dyn FnOnce(i32) -> i32>> = None;
        let mut kubectl_flags = Vec::new();


        while let Some(arg) = args_it.next() {
            match arg.as_str() {
                "--dry-run" => {
                    dry_run = true;
                },
                "deployment" => {
                    let v = consume_or_error(args_it, "expected name for deployment")?;
                    target.push_str("deployment/");
                    target.push_str(v.as_str());
                },
                "statefulset" => {
                    let v = consume_or_error(args_it, "expected name for statefulset")?;
                    target.push_str("statefulset/");
                    target.push_str(v.as_str());
                },
                "--replicas" => {
                    let v = consume_or_error(args_it, "expected value for replicas flag")?;
                    match v.parse::<i32>() {
                        Ok(replicas) =>  {
                            scale_op = Some(Box::from(move |_: i32| { replicas }));
                        },
                        Err(e) => {
                            return Err(format!("invalid value for --replcias {}", e));
                        }
                    }
                },
                "--help" => {
                    let s =r###"
                    kubectl scalex is a wrapper around kubectl scale, with the added functionality of expressing how much you want
                    to scale by instead of specifying a specific number.

                    For example, use the following to scale up by 50%:

                        kubectl scalex deployment/mything +50%
                    
                    Or the following to scale down by two replicas:

                        kubectl scalex deployment/mything -2

                    All flags, including --replicas, that work with kubectl-scale will also work with scalex. Use kubectl scale --help for more information.
                    "###;
                    return  Err(unindent(s));
                },
                _ => {
                    if is_single_kube_flag(arg) {
                        kubectl_flags.push(arg.to_string());
                    } else if is_valued_kube_flag(arg) {
                        let emsg = format!("expected value for {}", arg);
                        let v = consume_or_error(args_it, &emsg)?;
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
            return  Err(String::from("missing target (deployment or statefulset)"));
        }

        if scale_op.is_none() {
            eprint!("scaling operation was not specified");
            std::process::exit(1);
        }


        return  Ok(Parsed {
            kube_args: kubectl_flags,
            dry_run: dry_run, 
            scale_op: scale_op.unwrap(),
            target:target,
        });
    

    }
    
}



fn consume_or_error(it: &mut Iter<String>, err_msg: &str) -> Result<String, String>
{
    match it.next() {
        Some(v) => Ok(v.to_string()),
        None => {
            Err(String::from(err_msg))
        }
    }
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


fn unindent(source: &str) -> String {
    let mut lines = source.lines();
    lines.next(); // take first empty line
    let first_line = lines.next().unwrap();

    println!("first line {first_line}");
    let mut prefix = String::new();
    for c in first_line.chars() {
        if c.is_whitespace() {
            prefix.push(c);
        } else {
            break;
        }
    }
    println!("{} prefix", prefix);


    let mut unindented = String::new();
    unindented.push_str(first_line.strip_prefix(prefix.as_str()).unwrap());
    unindented.push('\n');

    for l in lines {
        unindented.push_str(l.strip_prefix(prefix.as_str()).unwrap_or(l));
        unindented.push('\n');
    }


    unindented


}



#[cfg(test)]
mod test {

    use super::*;

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
