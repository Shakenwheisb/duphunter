import { Badge, makeStyles, Text, tokens } from "@fluentui/react-components";
import { ShieldCheckmark20Regular } from "@fluentui/react-icons";

const useStyles = makeStyles({
  bar: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    padding: "10px 18px",
    borderBottom: `1px solid ${tokens.colorNeutralStroke2}`,
    backgroundColor: tokens.colorNeutralBackground1,
  },
  left: { display: "flex", alignItems: "center", gap: "10px" },
  logo: {
    width: "26px",
    height: "26px",
    borderRadius: "7px",
    background: `linear-gradient(135deg, ${tokens.colorBrandBackground}, ${tokens.colorPaletteBerryBackground3})`,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    color: "#fff",
    fontWeight: 700,
    fontSize: "15px",
  },
});

export function TitleBar() {
  const s = useStyles();
  return (
    <div className={s.bar}>
      <div className={s.left}>
        <div className={s.logo}>D</div>
        <Text weight="semibold" size={400}>
          DupHunter
        </Text>
        <Text size={200} style={{ opacity: 0.6 }}>
          local duplicate finder
        </Text>
      </div>
      <Badge
        appearance="tint"
        color="success"
        icon={<ShieldCheckmark20Regular />}
        size="large"
      >
        100% offline — nothing leaves this PC
      </Badge>
    </div>
  );
}
