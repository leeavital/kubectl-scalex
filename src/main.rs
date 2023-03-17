use std::vec;




fn main() {


    // TODO: make this static
    let flags = vec![
        ("-n", "--namespace", "--namespace="),
        ("-c", "--context", "--context="),

    ];

    let args : Vec<String> = std::env::args().collect();
    let mut args_it = args.iter();
    args_it.next();

    let mut kubectl_flags = Vec::new();
    let mut target = String::new();
    let mut dry_run = false;
    let mut scale_op : Option<Box<dyn FnOnce(i32) -> i32>> = None;

    while let Some(arg) = args_it.next() {

        let mut did_parse = false;
        for (short, long, single) in flags.iter() {
            if arg == short || arg == long {
                let flag_value = args_it.next().unwrap_or_else(|| {
                    eprintln!("missing value for {arg}");
                    std::process::exit(1);
                });
                kubectl_flags.push(arg.clone());
                kubectl_flags.push(flag_value.clone());
                did_parse = true;
            } else if arg.starts_with(single) {
                did_parse = true;
                kubectl_flags.push(arg.clone());
            }
        }
        if did_parse {
            continue;
        }

        if arg == "--dry-run" {
            dry_run = true;
        }

        else if arg == "deployment" {
            let d = args_it.next().unwrap_or_else(|| {
                eprint!("expected a deployment name");
                std::process::exit(1);
            });
            target.push_str("deployment/");
            target.push_str(d.as_str());
            continue;
        }

        else {
            match parse_op(&arg) {
                Some(s) => {
                    scale_op = Some(Box::from(s));
                },
                None => {
                    eprint!("could not parse operation: {}", &arg);
                    std::process::exit(1);
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
    }

    let kubectl_part = kubectl_flags.join(" ");
    println!("kubectl {kubectl_part} get {target}");
    let replicas_for_deployment = 60;
    let target_replicas = scale_op.unwrap()(replicas_for_deployment);
    println!("kubectl {kubectl_part} scale {target} --replicas {target_replicas}");

    println!("{:?}", args);
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
    use crate::parse_op;



    #[test]
    fn test_parse() {
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
}