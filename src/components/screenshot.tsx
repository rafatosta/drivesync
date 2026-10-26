import { useI18n } from '@/i18n/useI18n';
import { ArrowUpRight } from "lucide-react";
import { Dialog, DialogContent, DialogDescription, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
const base = import.meta.env.BASE_URL;
export function Screenshot({
  src,
  title,
  description
}: {
  src: string;
  title: string;
  description: string;
}) {
  const {
    t
  } = useI18n();
  return <figure className="screenshot"><Dialog><DialogTrigger render={<button className="screenshot-button" aria-label={t("Ampliar captura: {title}", {
        title
      })} />}><img src={`${base}screenshots/${src}`} alt={title} loading={src === 'dashboard.png' ? 'eager' : 'lazy'} /><span className="zoom-label">{t("Ampliar")} <ArrowUpRight size={14} /></span></DialogTrigger><DialogContent className="max-w-[min(96vw,1200px)] sm:max-w-[min(96vw,1200px)]"><DialogTitle>{title}</DialogTitle><DialogDescription>{description}</DialogDescription><img className="expanded-shot" src={`${base}screenshots/${src}`} alt={title} /></DialogContent></Dialog><figcaption>{description}</figcaption></figure>;
}
