/**
 * 邮件服务抽象层。
 *
 * 演示环境（当前）：无邮件外发能力，provider 为 noop——验证码由注册界面
 * 明示展示。接入真实邮件服务时：
 *
 *   import { setMailProvider } from "../lib/mail";
 *   setMailProvider({
 *     name: "resend", // 或 sendgrid / 自建 SMTP 桥
 *     async send(to, code) {
 *       await fetch("/api/mail", { method: "POST", body: JSON.stringify({ to, code }) });
 *     },
 *   });
 *
 * provider 非 "demo" 时界面不再显示验证码。
 */
export interface MailProvider {
  name: string;
  send: (to: string, code: string) => Promise<void>;
}

const demoProvider: MailProvider = {
  name: "demo",
  async send() {
    /* 演示环境：无外发，验证码由界面展示 */
  },
};

let current: MailProvider = demoProvider;

export function setMailProvider(p: MailProvider) {
  current = p;
}

export function getMailProvider(): MailProvider {
  return current;
}

export async function sendVerificationEmail(to: string, code: string) {
  await current.send(to, code);
}
