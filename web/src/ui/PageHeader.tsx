import type { ReactNode } from 'react'

interface PageHeaderProps {
  title: string
  kicker: string
  description?: string
  actions?: ReactNode
  status?: ReactNode
}

export default function PageHeader({ title, kicker, description, actions, status }: PageHeaderProps) {
  return (
    <div className="page-header">
      <div className="page-header__copy">
        <span className="page-header__kicker">{kicker}</span>
        <h1 className="page-header__title">{title}</h1>
        {description ? <p className="page-header__desc">{description}</p> : null}
      </div>
      {(status || actions) ? (
        <div className="page-header__aside">
          {status ? <div className="page-header__status">{status}</div> : null}
          {actions ? <div className="page-header__actions">{actions}</div> : null}
        </div>
      ) : null}
    </div>
  )
}
