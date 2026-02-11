import { ChangeDetectionStrategy, Component, Input } from '@angular/core';
import { DomSanitizer, SafeHtml } from '@angular/platform-browser';
import { getIcon, IconName } from '../../icons';

@Component({
  selector: 'app-icon',
  standalone: true,
  template: `<span class="icon" [innerHTML]="svg"></span>`,
  styles: [`.icon { display: inline-flex; line-height: 0; }`],
  changeDetection: ChangeDetectionStrategy.OnPush
})
export class IconComponent {
  @Input({ required: true }) name!: IconName;
  @Input() size: number = 24;

  get svg(): SafeHtml {
    return this.sanitizer.bypassSecurityTrustHtml(getIcon(this.name, this.size));
  }

  constructor(private sanitizer: DomSanitizer) {}
}
