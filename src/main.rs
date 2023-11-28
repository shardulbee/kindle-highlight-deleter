use std::process::{Child, Command, Stdio};
use fantoccini::{ClientBuilder, Locator};

use std::time::Duration;
use std::thread::sleep;

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        match self.0.kill() {
            Err(e) => println!("Could not kill child process: {}", e),
            Ok(_) => println!("Successfully killed child process"),
        }
    }
}


// let's set up the sequence of steps we want the browser to take
#[tokio::main]
async fn main() -> Result<(), fantoccini::error::CmdError> {
    let driver = Command::new("geckodriver")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn geckodriver");
    let _guard = ChildGuard(driver);

    let email = get_login_info("email".to_string()).await;
    let password = get_login_info("password".to_string()).await;

    let c = ClientBuilder::native().connect("http://localhost:4444").await.expect("failed to connect to WebDriver");

    c.goto("https://read.amazon.com/landing").await?;
    let url = c.current_url().await?;
    assert_eq!(url.as_ref(), "https://read.amazon.com/landing");

    c.find(Locator::Id("top-sign-in-btn")).await?.click().await?;

    let f = c.form(Locator::Css("form[name='signIn']")).await?;
    f.set_by_name("email", &email).await?;
    f.set_by_name("password", &password).await?;
    c.find(Locator::Id("signInSubmit")).await?.click().await?;

    let otp = get_login_info("one-time password?attribute=otp".to_string()).await;
    let f = c.form(Locator::Id("auth-mfa-form")).await?;
    f.set_by_name("otpCode", &otp).await?;
    c.find(Locator::Id("auth-signin-button")).await?.click().await?;

    c.goto("https://read.amazon.com/notebook?ref_=kcr_notebook_lib&language=en-US").await?;

    // need to sleep because all the books are dynamically fetched after page load
    sleep(Duration::from_secs(5));

    // Vec<Element>
    let res = c.find_all(Locator::Css(".kp-notebook-searchable.a-text-bold")).await?;

    println!("res: {:?}", res.len());

    for element in &res {
        let mut element_click_result = element.click().await;
        while element_click_result.is_err() {
            sleep(Duration::from_secs(2));
            element_click_result = element.click().await;
        }

        println!("book title: {}", element.text().await?);
        let all_highlights = c.wait().for_element(Locator::Css("#kp-notebook-annotations")).await?;


        let highlights_count = all_highlights.find_all(Locator::Css("a.a-popover-trigger.a-declarative")).await?.len();
        for _i in 0..highlights_count {
            let popover_toggle = c.wait().for_element(Locator::Css("a.a-popover-trigger.a-declarative[id^=popover-]")).await?;
            // while !popover_toggle.is_displayed().await? {
            //     sleep(Duration::from_secs(1));
            // }
            let mut popover_click_result = popover_toggle.click().await;
            while popover_click_result.is_err() {
                sleep(Duration::from_secs(2));
                popover_click_result = popover_toggle.click().await;
            }

            let popover_delete_button = c.wait().for_element(Locator::Css("a[id^='delete']")).await?;
            let popover_button_id = popover_delete_button.attr("id").await?.unwrap();
            let note_or_highlight = match popover_button_id.as_str() {
                "deletenote" => NoteOrHighlight::Note,
                "deletehighlight" => NoteOrHighlight::Highlight,
                _ => panic!("unexpected popover_button_id: {}", popover_button_id),
            };
            println!("parsed: {}", note_or_highlight);
            popover_delete_button.click().await?;

            let modal_delete_button_id = format!("delete{}", note_or_highlight);
            let modal_delete_button = c.wait().for_element(Locator::Css(format!("span#{}", modal_delete_button_id).as_str())).await?;
            while modal_delete_button.is_displayed().await? {
                modal_delete_button.click().await?;
            }
        }
        println!();
    }

    c.close().await
}

enum NoteOrHighlight {
    Note,
    Highlight,
}

impl std::fmt::Display for NoteOrHighlight {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            NoteOrHighlight::Note => write!(f, "Note"),
            NoteOrHighlight::Highlight => write!(f, "Highlight"),
        }
    }
}

async fn get_login_info(component: String) -> String {
    let output = Command::new("op")
        .args(["read", &format!("op://Personal/Amazon/{}", component)])
        .stdout(Stdio::piped())
        .output()
        .expect("Failed to execute command");

    // Capture the stdout of the command as a string
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    stdout
}
