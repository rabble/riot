class Calendar < ActiveRecord::Base
  has_many :events
  
  def events_on_day(day)
    events.on_day(day)
  end

  def events_in_month(month)
    events.in_month(month)
  end

end
