import { NavLink, Outlet } from "react-router";
import { useAppearance } from "./hooks/useAppearance";
import {
  Theme,
  Flex,
  TabNav,
  IconButton,
  Button,
  Tooltip,
  Text,
} from "@radix-ui/themes";
import { SunIcon, MoonIcon, GitHubLogoIcon } from "@radix-ui/react-icons";

export default function Layout() {
  const [appearance, toggleAppearance] = useAppearance();

  return (
    <Theme accentColor="purple" radius="large" appearance={appearance}>
      <Flex direction="column" height="100vh">
        <Flex
          align="center"
          justify="between"
          px="4"
          style={{ borderBottom: "1px solid var(--gray-5)" }}
        >
          <Flex align="center" gap="4">
            <Text size="3" weight="bold">
              ossm-rs
            </Text>
            <TabNav.Root>
              <NavLink to="/" end>
                {({ isActive }) => (
                  <TabNav.Link asChild active={isActive}>
                    <span>Simulator</span>
                  </TabNav.Link>
                )}
              </NavLink>
              <NavLink to="/firmware">
                {({ isActive }) => (
                  <TabNav.Link asChild active={isActive}>
                    <span>Firmware</span>
                  </TabNav.Link>
                )}
              </NavLink>
              <NavLink to="/debugging">
                {({ isActive }) => (
                  <TabNav.Link asChild active={isActive}>
                    <span>Debugging</span>
                  </TabNav.Link>
                )}
              </NavLink>
            </TabNav.Root>
          </Flex>
          <Flex align="center" gap="2">
            <Button variant="soft" size="2" asChild>
              <a
                href="https://github.com/ossm-rs/ossm"
                target="_blank"
                rel="noopener noreferrer"
              >
                <GitHubLogoIcon /> ossm-rs/ossm
              </a>
            </Button>
            <Tooltip
              content={
                appearance === "light"
                  ? "Switch to dark mode"
                  : "Switch to light mode"
              }
            >
              <IconButton
                variant="soft"
                size="2"
                onClick={toggleAppearance}
                aria-label="Toggle theme"
              >
                {appearance === "light" ? <SunIcon /> : <MoonIcon />}
              </IconButton>
            </Tooltip>
          </Flex>
        </Flex>
        <Outlet />
      </Flex>
    </Theme>
  );
}
