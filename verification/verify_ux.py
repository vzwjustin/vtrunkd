
from playwright.sync_api import sync_playwright, expect
import re

def test_ux_improvements(page):
    page.goto("http://localhost:3000")

    # 1. Verify Loading State
    # We need to mock the Tauri invoke function to simulate a delay
    page.evaluate("""
        window.__TAURI_IPC__ = async (message) => {
            console.log('IPC call:', message);
            // Simulate delay for any invoke call
            await new Promise(resolve => setTimeout(resolve, 2000));
            return {
                client_yaml: "mock client yaml",
                server_yaml: "mock server yaml",
                client_public_key: "mock key",
                server_public_key: "mock key"
            };
        };
    """)

    # Click generate and immediately screenshot to capture loading state
    generate_btn = page.get_by_role("button", name="Generate configs")
    generate_btn.click()

    # Wait a tiny bit for the class to apply
    page.wait_for_timeout(500)

    # Verify loading class is present (button text should be transparent, spinner visible via CSS)
    expect(generate_btn).to_have_class(re.compile(r"loading"))

    # Take screenshot
    page.screenshot(path="verification/ux_verification_refined.png")

if __name__ == "__main__":
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page()
        try:
            test_ux_improvements(page)
        except Exception as e:
            print(f"Test failed: {e}")
        finally:
            browser.close()
